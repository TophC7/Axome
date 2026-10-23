use crate::errors::ApiError;
use crate::services::env::EnvironmentService;
use crate::services::folders::home_dir;
use crate::services::history::omp::session_bucket;
use std::path::{Path, PathBuf};
use sworm_protocol::omp::OmpResolvedTarget;

fn latest_session_paths(cwd: &str) -> Option<(PathBuf, PathBuf)> {
    latest_in(session_bucket(cwd)?)
}

fn latest_in(bucket: PathBuf) -> Option<(PathBuf, PathBuf)> {
    let entries = std::fs::read_dir(&bucket).ok()?;
    let latest = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".jsonl") {
                Some(name)
            } else {
                None
            }
        })
        .max()?;
    let stem = latest.strip_suffix(".jsonl")?;
    let jsonl_file = bucket.join(&latest);
    let artifacts_dir = bucket.join(stem);
    Some((jsonl_file, artifacts_dir))
}

fn find_any_latest_session() -> Option<(PathBuf, PathBuf)> {
    let home = home_dir()?;
    let sessions_root = home.join(".omp").join("agent").join("sessions");
    std::fs::read_dir(sessions_root)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| latest_in(entry.path()))
        .max_by(|left, right| left.0.file_name().cmp(&right.0.file_name()))
}

fn fallback_omp_read(uri: &str, home: &Path, slug: &str) -> Result<OmpResolvedTarget, ApiError> {
    let cache_dir = home.join(".cache").join("sworm").join("omp-docs");
    let safe_slug = slug.replace(['/', ':', ' '], "-");
    let doc_file = cache_dir.join(format!("{safe_slug}.md"));
    if doc_file.is_file() {
        return Ok(OmpResolvedTarget {
            path: doc_file.to_string_lossy().into_owned(),
            is_dir: false,
        });
    }
    let output = std::process::Command::new("omp")
        .env_clear()
        .envs(&EnvironmentService::current().base)
        .args(["read", uri])
        .output()
        .map_err(|e| ApiError::Io(format!("Failed to run omp read: {e}")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let msg = if !err.trim().is_empty() { err } else { out };
        return Err(ApiError::Internal(msg.trim().to_string()));
    }
    let text = String::from_utf8(output.stdout).map_err(|e| ApiError::Internal(e.to_string()))?;
    std::fs::create_dir_all(&cache_dir)?;
    std::fs::write(&doc_file, text)?;
    Ok(OmpResolvedTarget {
        path: doc_file.to_string_lossy().into_owned(),
        is_dir: false,
    })
}

pub fn resolve_omp_target(uri: &str, cwd: Option<&str>) -> Result<OmpResolvedTarget, ApiError> {
    let home =
        home_dir().ok_or_else(|| ApiError::NotFound("HOME directory not set".to_string()))?;
    let (scheme, rest) = uri
        .split_once("://")
        .ok_or_else(|| ApiError::InvalidArgument(format!("Invalid URI scheme: {uri}")))?;
    let target_raw = rest.split('?').next().unwrap_or(rest);
    let target_base = target_raw.split(':').next().unwrap_or(target_raw);
    let target = target_base.trim_matches('/');

    let get_session = || {
        cwd.and_then(latest_session_paths)
            .or_else(find_any_latest_session)
    };

    fn file(path: PathBuf) -> Option<OmpResolvedTarget> {
        path.is_file().then(|| OmpResolvedTarget {
            path: path.to_string_lossy().into_owned(),
            is_dir: false,
        })
    }

    fn entry(path: PathBuf) -> Option<OmpResolvedTarget> {
        path.exists().then(|| OmpResolvedTarget {
            is_dir: path.is_dir(),
            path: path.to_string_lossy().into_owned(),
        })
    }

    let resolved = match scheme {
        "history" => get_session().and_then(|(jsonl, artifacts)| {
            if target.is_empty() || target == "Main" {
                file(jsonl)
            } else {
                file(artifacts.join(format!("{target}.jsonl"))).or_else(|| file(jsonl))
            }
        }),
        "agent" => get_session()
            .and_then(|(jsonl, artifacts)| {
                if target.is_empty() || target == "Main" {
                    file(jsonl)
                } else {
                    file(artifacts.join(format!("{target}.md")))
                }
            })
            .or_else(|| file(home.join(".omp/agent/agents").join(format!("{target}.md"))))
            .or_else(|| {
                cwd.and_then(|cwd| {
                    file(
                        Path::new(cwd)
                            .join("omp/agents")
                            .join(format!("{target}.md")),
                    )
                    .or_else(|| {
                        file(
                            Path::new(cwd)
                                .join(".omp/agents")
                                .join(format!("{target}.md")),
                        )
                    })
                })
            }),
        "artifact" => get_session().and_then(|(_, artifacts)| {
            let entries = std::fs::read_dir(&artifacts).ok()?.flatten();
            if target == "latest" {
                let mut best: Option<(u32, PathBuf)> = None;
                for item in entries {
                    let name = item.file_name();
                    let name = name.to_string_lossy();
                    let Some(number) = name
                        .split('.')
                        .next()
                        .and_then(|part| part.parse::<u32>().ok())
                    else {
                        continue;
                    };
                    if best.as_ref().is_none_or(|(previous, _)| number > *previous) {
                        best = Some((number, item.path()));
                    }
                }
                best.and_then(|(_, path)| file(path))
            } else {
                let prefix = format!("{target}.");
                entries
                    .filter_map(|item| {
                        let name = item.file_name();
                        let name = name.to_string_lossy();
                        (name.starts_with(&prefix) || name == target).then(|| item.path())
                    })
                    .find_map(file)
            }
        }),
        "local" => get_session()
            .and_then(|(_, artifacts)| {
                file(artifacts.join("local").join(target)).or_else(|| file(artifacts.join(target)))
            })
            .or_else(|| {
                cwd.and_then(|cwd| {
                    file(Path::new(cwd).join(".omp").join(target))
                        .or_else(|| file(Path::new(cwd).join(target)))
                })
            }),
        "skill" => {
            let (name, subpath) = target.split_once('/').unwrap_or((target, "SKILL.md"));
            let relative = if subpath.is_empty() {
                "SKILL.md"
            } else {
                subpath
            };
            std::fs::read_dir(home.join(".omp/plugins/node_modules"))
                .ok()
                .and_then(|entries| {
                    entries.flatten().find_map(|item| {
                        entry(item.path().join("skills").join(name).join(relative))
                    })
                })
                .or_else(|| entry(home.join(".omp/agent/skills").join(name).join(relative)))
                .or_else(|| {
                    cwd.and_then(|cwd| {
                        entry(Path::new(cwd).join(".omp/skills").join(name).join(relative))
                    })
                })
        }
        "rule" => cwd
            .and_then(|cwd| {
                file(
                    Path::new(cwd)
                        .join("omp/extensions")
                        .join(format!("{target}.ts")),
                )
                .or_else(|| {
                    file(
                        Path::new(cwd)
                            .join(".omp/rules")
                            .join(format!("{target}.md")),
                    )
                })
            })
            .or_else(|| {
                file(
                    home.join(".omp/agent/extensions")
                        .join(format!("{target}.ts")),
                )
            })
            .or_else(|| file(home.join(".omp/agent/rules").join(format!("{target}.md")))),
        "omp" => {
            let slug = if target.is_empty() { "index" } else { target };
            return fallback_omp_read(uri, &home, slug);
        }
        _ => {
            return Err(ApiError::InvalidArgument(format!(
                "Unsupported scheme: {scheme}"
            )))
        }
    };
    resolved.map_or_else(|| fallback_omp_read(uri, &home, target), Ok)
}
