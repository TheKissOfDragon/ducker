use bollard::Docker;
use color_eyre::eyre::{Context, Result};
use ratatui::{
    layout::{Alignment, Margin, Rect},
    widgets::{block::Title, Block, Padding},
    Frame,
};
use tokio::sync::mpsc::Sender;

use crate::{
    context::AppContext,
    events::{message::MessageResponse, Key, Message, Transition},
    pages::{attach::Attach, containers::Containers, images::Images, logs::Logs},
    state,
    traits::{Component, Page},
};

#[derive(Debug)]
pub struct PageManager {
    current_page: state::CurrentPage,
    page: Box<dyn Page>,
    tx: Sender<Message<Key, Transition>>,
    docker: Docker,
}

impl PageManager {
    pub async fn new(
        page: state::CurrentPage,
        tx: Sender<Message<Key, Transition>>,
    ) -> Result<Self> {
        let docker = bollard::Docker::connect_with_socket_defaults()
            .context("unable to connect to local docker daemon")?;

        let containers = Box::new(Containers::new(docker.clone(), tx.clone()));

        let mut page_manager = Self {
            current_page: page,
            page: containers,
            tx,
            docker,
        };

        page_manager
            .init()
            .await
            .context("failed to initialise page manager")?;

        Ok(page_manager)
    }

    pub async fn init(&mut self) -> Result<()> {
        self.page.set_visible(AppContext::default()).await?;
        Ok(())
    }

    pub async fn transition(&mut self, transition: Transition) -> Result<MessageResponse> {
        let result = match transition {
            Transition::ToImagePage(cx) => {
                self.set_current_page(state::CurrentPage::Images, cx)
                    .await?;
                MessageResponse::Consumed
            }
            Transition::ToContainerPage(cx) => {
                self.set_current_page(state::CurrentPage::Containers, cx)
                    .await?;
                MessageResponse::Consumed
            }
            Transition::ToLogPage(cx) => {
                self.set_current_page(state::CurrentPage::Logs, cx).await?;
                MessageResponse::Consumed
            }
            Transition::ToAttach(cx) => {
                self.set_current_page(state::CurrentPage::Attach, cx)
                    .await?;
                MessageResponse::Consumed
            }
            _ => MessageResponse::NotConsumed,
        };
        Ok(result)
    }

    pub async fn update(&mut self, message: Key) -> Result<MessageResponse> {
        self.page.update(message).await
    }

    async fn set_current_page(
        &mut self,
        next_page: state::CurrentPage,
        cx: AppContext,
    ) -> Result<()> {
        if next_page == self.current_page {
            return Ok(());
        }
        self.page
            .set_invisible()
            .await
            .context("unable to close old page")?;

        self.current_page = next_page.clone();

        match next_page {
            state::CurrentPage::Attach => self.page = Box::new(Attach::new(self.tx.clone())),
            state::CurrentPage::Containers => {
                self.page = Box::new(Containers::new(self.docker.clone(), self.tx.clone()))
            }
            state::CurrentPage::Images => self.page = Box::new(Images::new(self.docker.clone())),
            state::CurrentPage::Logs => {
                self.page = Box::new(Logs::new(self.docker.clone(), self.tx.clone()))
            }
        };

        self.page
            .set_visible(cx)
            .await
            .context("unable to open new page")?;

        Ok(())
    }

    pub fn draw_help(&mut self, f: &mut Frame<'_>, area: Rect) {
        self.page.get_help().lock().unwrap().draw(f, area);
    }
}

impl Component for PageManager {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        let title_message = self.page.get_help().lock().unwrap().get_name();

        let title = Title::from(format!("< {} >", title_message)).alignment(Alignment::Center);

        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Plain)
            .title(title)
            .padding(Padding::left(300));

        f.render_widget(block, area);

        let inner_body_margin = Margin::new(2, 1);
        let body_inner = area.inner(inner_body_margin);

        self.page.draw(f, body_inner);
    }
}
