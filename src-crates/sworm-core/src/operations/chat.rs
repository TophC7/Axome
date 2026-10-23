use crate::{errors::ApiError, events::EventSink, services::folders::resolve_folder, Host};
use sworm_protocol::chat::{ChatAnswer, ChatEvent, ChatForkInfo, ChatModelCatalog, ChatThinking};

impl Host {
    pub fn chat_models(&self, folder_path: String) -> Result<ChatModelCatalog, ApiError> {
        self.axo.bind_runtime();
        let _folder = resolve_folder(&folder_path)?;
        self.axo.block_on(self.axo.catalog())?
    }

    pub fn chat_create(
        &self,
        folder_path: String,
        model: String,
        thinking: ChatThinking,
    ) -> Result<String, ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.create(&folder, model, thinking))?
    }

    pub fn chat_prompt(
        &self,
        folder_path: String,
        session_id: String,
        request_id: String,
        text: String,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.prompt(&folder, &session_id, request_id, text))?
    }

    pub fn chat_steer(
        &self,
        folder_path: String,
        session_id: String,
        text: String,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.steer(&folder, &session_id, text))?
    }

    pub fn chat_follow_up(
        &self,
        folder_path: String,
        session_id: String,
        text: String,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.follow_up(&folder, &session_id, text))?
    }

    pub fn chat_abort(&self, folder_path: String, session_id: String) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo.block_on(self.axo.abort(&folder, &session_id))?
    }

    pub fn chat_answer(
        &self,
        folder_path: String,
        session_id: String,
        call_id: String,
        answers: Vec<ChatAnswer>,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.answer(&folder, &session_id, call_id, answers))?
    }

    pub fn chat_dismiss(
        &self,
        folder_path: String,
        session_id: String,
        call_id: String,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.dismiss(&folder, &session_id, call_id))?
    }

    pub fn chat_set_model(
        &self,
        folder_path: String,
        session_id: String,
        model: String,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.set_model(&folder, &session_id, model))?
    }

    pub fn chat_set_thinking(
        &self,
        folder_path: String,
        session_id: String,
        thinking: ChatThinking,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.set_thinking(&folder, &session_id, thinking))?
    }

    pub fn chat_compact(&self, folder_path: String, session_id: String) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo.block_on(self.axo.compact(&folder, &session_id))?
    }

    pub fn chat_navigate(
        &self,
        folder_path: String,
        session_id: String,
        entry_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.navigate(&folder, &session_id, entry_id))?
    }

    pub fn chat_select_branch(
        &self,
        folder_path: String,
        session_id: String,
        entry_id: String,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.select_branch(&folder, &session_id, entry_id))?
    }

    pub fn chat_fork(
        &self,
        folder_path: String,
        session_id: String,
        entry_id: String,
    ) -> Result<ChatForkInfo, ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.fork(&folder, &session_id, entry_id))?
    }

    pub fn chat_attach(
        &self,
        folder_path: String,
        session_id: String,
        attach_id: String,
        events: EventSink<ChatEvent>,
    ) -> Result<(), ApiError> {
        self.axo.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.axo
            .block_on(self.axo.attach(&folder, &session_id, attach_id, events))?
    }

    pub fn chat_detach(&self, session_id: &str, attach_id: &str) {
        self.axo.detach(session_id, attach_id);
    }
}
