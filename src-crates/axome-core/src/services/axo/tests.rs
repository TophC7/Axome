use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axo::{Axo, Settings};
use axo_ai::testing::{reply_text, reply_tool_calls};
use axo_ai::{FileCredentialStore, Models, ModelsConfig, ModelsOptions, ScriptedApi, ScriptedTurn};
use parking_lot::Mutex;
use serde_json::json;
use axome_protocol::chat::{
    ChatAnswer, ChatEntry, ChatEntryBody, ChatEvent, ChatRunOutcome, ChatThinking,
};

use super::AxoService;
use crate::events::EventSink;
use crate::services::db::DatabaseService;

type Events = Arc<Mutex<Vec<ChatEvent>>>;

struct Fixture {
    service: Arc<AxoService>,
    folder: PathBuf,
    dir: PathBuf,
}

impl Fixture {
    async fn new(turns: Vec<ScriptedTurn>) -> Self {
        let dir = std::env::temp_dir().join(format!("axome-axo-{}", uuid::Uuid::new_v4()));
        let folder = dir.join("work");
        std::fs::create_dir_all(&folder).unwrap();
        let config = ModelsConfig::parse_jsonc(
            &json!({"providers": {"test": {
                "api": "openai-completions",
                "baseUrl": "http://127.0.0.1:9/v1",
                "models": [{"id": "m"}, {"id": "m2"}]
            }}})
            .to_string(),
        )
        .unwrap();
        let models = Arc::new(
            Models::load(ModelsOptions {
                cache_dir: None,
                config,
                extended_context: false,
                credentials: Arc::new(FileCredentialStore::new(dir.join("auth.json"))),
            })
            .await
            .unwrap(),
        );
        let axo = Axo::builder()
            .dir(dir.clone())
            .settings(Settings::default())
            .models(models)
            .streamer(Arc::new(ScriptedApi::new(turns)))
            .build()
            .await
            .unwrap();
        let db = Arc::new(DatabaseService::new(dir.join("test.db")).unwrap());
        let service = AxoService::with_axo(db, Arc::new(axo));
        service.bind_runtime();
        Self {
            service,
            folder,
            dir,
        }
    }

    async fn create(&self) -> String {
        self.service
            .create(&self.folder, "test/m".into(), ChatThinking::Off)
            .await
            .unwrap()
    }

    async fn attach(&self, id: &str, attach_id: &str) -> Events {
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = Arc::clone(&events);
        let sink: EventSink<ChatEvent> = Arc::new(move |event| {
            collected.lock().push(event);
            Ok(())
        });
        self.service
            .attach(&self.folder, id, attach_id.into(), sink)
            .await
            .unwrap();
        events
    }

    async fn prompt(&self, id: &str, text: &str, events: &Events, runs: usize) {
        self.service
            .prompt(&self.folder, id, format!("request-{text}"), text.into())
            .await
            .unwrap();
        wait_until(|| settled(&events.lock(), runs)).await;
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.service.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn wait_until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timed out waiting for axo events");
}

fn settled(events: &[ChatEvent], runs: usize) -> bool {
    let done = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ChatEvent::Entry {
                    entry: ChatEntry {
                        body: ChatEntryBody::RunEnd {
                            outcome: ChatRunOutcome::Done,
                            ..
                        },
                        ..
                    }
                }
            )
        })
        .count();
    done == runs
        && events.iter().rev().find_map(|event| match event {
            ChatEvent::Live { live } => Some(!live.running && live.questions.is_empty()),
            ChatEvent::Snapshot { state } => {
                Some(!state.live.running && state.live.questions.is_empty())
            }
            _ => None,
        }) == Some(true)
}

fn entries(events: &[ChatEvent]) -> Vec<ChatEntry> {
    let mut entries = Vec::new();
    for event in events {
        match event {
            ChatEvent::Snapshot { state } => entries.clone_from(&state.entries),
            ChatEvent::Entry { entry } => entries.push(entry.clone()),
            _ => {}
        }
    }
    entries
}

fn last_user(entries: &[ChatEntry]) -> &ChatEntry {
    entries
        .iter()
        .rev()
        .find(|entry| matches!(entry.body, ChatEntryBody::User { .. }))
        .expect("user entry")
}

#[tokio::test(flavor = "multi_thread")]
async fn two_subscribers_see_same_stream() {
    let fixture = Fixture::new(vec![ScriptedTurn::Reply(reply_text("hello"))]).await;
    let id = fixture.create().await;
    let a = fixture.attach(&id, "a").await;
    let b = fixture.attach(&id, "b").await;
    fixture.prompt(&id, "hi", &a, 1).await;
    wait_until(|| settled(&b.lock(), 1)).await;
    let a = a.lock();
    let b = b.lock();
    assert!(matches!(a.first(), Some(ChatEvent::Snapshot { .. })));
    assert!(matches!(b.first(), Some(ChatEvent::Snapshot { .. })));
    let user = a
        .iter()
        .position(|event| {
            matches!(event,
                ChatEvent::Entry { entry: ChatEntry { body: ChatEntryBody::User { text, .. }, .. } }
                if text == "hi"
            )
        })
        .expect("user entry");
    let assistant = a
        .iter()
        .position(|event| {
            matches!(
                event,
                ChatEvent::Entry {
                    entry: ChatEntry {
                        body: ChatEntryBody::Assistant { .. },
                        ..
                    }
                }
            )
        })
        .expect("assistant entry");
    let end = a
        .iter()
        .position(|event| {
            matches!(
                event,
                ChatEvent::Entry {
                    entry: ChatEntry {
                        body: ChatEntryBody::RunEnd {
                            outcome: ChatRunOutcome::Done,
                            ..
                        },
                        ..
                    }
                }
            )
        })
        .expect("successful run end");
    assert!(user < assistant && assistant < end);
    assert_eq!(*a, *b);
}

#[tokio::test(flavor = "multi_thread")]
async fn ask_round_trip() {
    let fixture = Fixture::new(vec![
        ScriptedTurn::Reply(reply_tool_calls(&[(
            "ask-1",
            "ask",
            json!({
                "questions": [{
                    "id": "choice", "question": "Which file?",
                    "options": [{"label": "first"}, {"label": "second"}]
                }]
            }),
        )])),
        ScriptedTurn::Reply(reply_text("Opening the first file.")),
    ])
    .await;
    let id = fixture.create().await;
    let events = fixture.attach(&id, "a").await;
    fixture
        .service
        .prompt(&fixture.folder, &id, "ask".into(), "Choose a file".into())
        .await
        .unwrap();
    wait_until(|| {
        events.lock().iter().any(|event| {
            matches!(event,
                ChatEvent::Live { live } if live.questions.len() == 1
            )
        })
    })
    .await;
    let pending = events
        .lock()
        .iter()
        .rev()
        .find_map(|event| match event {
            ChatEvent::Live { live } if live.questions.len() == 1 => {
                Some(live.questions[0].clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(pending.questions.len(), 1);
    let question = &pending.questions[0];
    fixture
        .service
        .answer(
            &fixture.folder,
            &id,
            pending.call_id.clone(),
            vec![ChatAnswer {
                id: question.id.clone(),
                selected: vec![question.options[0].label.clone()],
                custom: None,
            }],
        )
        .await
        .unwrap();
    wait_until(|| settled(&events.lock(), 1)).await;
    let events = events.lock();
    let live = events
        .iter()
        .rev()
        .find_map(|event| match event {
            ChatEvent::Live { live } => Some(live),
            _ => None,
        })
        .unwrap();
    assert!(live.questions.is_empty());
    assert!(entries(&events).iter().any(|entry| matches!(&entry.body,
        ChatEntryBody::ToolResult { call_id, is_error: false, .. } if call_id == "ask-1"
    )));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_creates_sibling_and_switches_back() {
    let fixture = Fixture::new(vec![
        ScriptedTurn::Reply(reply_text("first reply")),
        ScriptedTurn::Reply(reply_text("second reply")),
        ScriptedTurn::Reply(reply_text("edited reply")),
    ])
    .await;
    let id = fixture.create().await;
    let events = fixture.attach(&id, "a").await;
    fixture.prompt(&id, "one", &events, 1).await;
    fixture.prompt(&id, "two", &events, 2).await;
    let two = last_user(&entries(&events.lock())).clone();
    let ChatEntryBody::User { text, anchor, .. } = &two.body else {
        unreachable!()
    };
    assert_eq!(text, "two");
    fixture
        .service
        .navigate(&fixture.folder, &id, anchor.clone())
        .await
        .unwrap();
    fixture.prompt(&id, "three", &events, 3).await;

    // A fresh subscriber must see the edited branch and all its sibling metadata.
    let edited = fixture.attach(&id, "edited").await;
    let snapshot = edited.lock();
    let Some(ChatEvent::Snapshot { state }) = snapshot.first() else {
        panic!("attach must begin with a snapshot");
    };
    let ChatEntryBody::User {
        text,
        anchor: edited_anchor,
        siblings,
        ..
    } = &last_user(&state.entries).body
    else {
        unreachable!()
    };
    assert_eq!(text, "three");
    assert_eq!(edited_anchor, anchor);
    assert_eq!(siblings.len(), 2);
    assert!(siblings.contains(&two.id));
    drop(snapshot);

    fixture
        .service
        .select_branch(&fixture.folder, &id, two.id)
        .await
        .unwrap();
    wait_until(|| {
        events.lock().iter().rev().find_map(|event| match event {
            ChatEvent::Snapshot { state } => Some(matches!(&last_user(&state.entries).body,
                ChatEntryBody::User { text, .. } if text == "two"
            )),
            _ => None,
        }) == Some(true)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn idle_session_unloads_and_reopens() {
    let fixture = Fixture::new(vec![ScriptedTurn::Reply(reply_text("hello"))]).await;
    let id = fixture.create().await;
    let events = fixture.attach(&id, "a").await;
    fixture.prompt(&id, "hi", &events, 1).await;
    let before = entries(&events.lock());
    fixture.service.detach(&id, "a");
    wait_until(|| fixture.service.loaded() == 0).await;
    assert_eq!(fixture.service.loaded(), 0);
    let reopened = fixture.attach(&id, "reopened").await;
    let reopened = reopened.lock();
    let Some(ChatEvent::Snapshot { state }) = reopened.first() else {
        panic!("reopen must begin with a snapshot");
    };
    assert_eq!(state.session_id, id);
    assert_eq!(state.entries, before);
    assert!(!state.live.running);
    assert_eq!(fixture.service.loaded(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_history_reads_title_and_model() {
    let fixture = Fixture::new(vec![ScriptedTurn::Reply(reply_text("hello"))]).await;
    let id = fixture.create().await;
    let events = fixture.attach(&id, "a").await;
    fixture.prompt(&id, "hi", &events, 1).await;
    fixture
        .service
        .set_model(&fixture.folder, &id, "test/m2".into())
        .await
        .unwrap();
    let service = Arc::clone(&fixture.service);
    let folder = fixture.folder.clone();
    let listing = tokio::task::spawn_blocking(move || service.list_history(&folder))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(listing.rows.len(), 1);
    let row = &listing.rows[0];
    assert_eq!(row.id, id);
    assert_eq!(row.title.as_deref(), Some("hi"));
    assert_eq!(
        row.last_message,
        Some(axome_protocol::session::LastMessage {
            role: axome_protocol::session::MessageRole::Agent,
            text: "hello".into(),
        })
    );
    assert_eq!(row.message_count, Some(2));
    assert_eq!(row.model.as_deref(), Some("test/m"));
    assert!(row.created_at.is_some());
    assert!(row.updated_at.is_some());
}

#[test]
fn list_history_rejects_a_stopped_runtime() {
    let dir = std::env::temp_dir().join(format!("axome-axo-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(DatabaseService::new(dir.join("test.db")).unwrap());
    let fixture = Fixture {
        service: AxoService::new(db),
        folder: dir.clone(),
        dir,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    {
        let _entered = runtime.enter();
        fixture.service.bind_runtime();
    }
    drop(runtime);
    let error = fixture.service.list_history(&fixture.folder).unwrap_err();
    assert!(error.contains("cancelled"), "{error}");
}
