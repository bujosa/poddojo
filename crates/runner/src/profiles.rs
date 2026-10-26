//! Learner profiles: several people (or several study plans) on one computer, each with their own
//! progress, plus a guest mode for practising without touching anyone's stats.
//!
//! ```text
//! <data>/profiles.json                 the list and the last one used
//! <data>/profiles/<id>/progress.json   each profile's progress
//! <data>/profiles/guest/               the guest's, wiped every time guest mode starts
//! ```
//!
//! Machines, the lab cluster and lab folders are shared: they are this computer's.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

pub const GUEST: &str = "guest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Index into the avatar palette.
    #[serde(default)]
    pub color: u8,
    #[serde(default)]
    pub created_at: i64,
}

impl Profile {
    pub fn guest() -> Self {
        Self { id: GUEST.into(), name: "Guest".into(), color: 7, created_at: 0 }
    }

    pub fn is_guest(&self) -> bool {
        self.id == GUEST
    }

    /// Up to two initials for the avatar.
    pub fn initials(&self) -> String {
        let mut s: String = self.name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect();
        if s.is_empty() {
            s.push('?');
        }
        s.to_uppercase()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Index {
    profiles: Vec<Profile>,
    last: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Profiles {
    base: Paths,
    index: Index,
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() { "learner".into() } else { s }
}

/// The name of the person logged in to this computer (`id -F` on macOS), else `$USER`.
pub fn system_name() -> String {
    std::process::Command::new("id")
        .arg("-F")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|n| !n.is_empty())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "Me".into())
}

impl Profiles {
    fn index_path(base: &Paths) -> PathBuf {
        base.root.join("profiles.json")
    }

    /// Load the profile list. Progress from before profiles existed (`<data>/progress.json`)
    /// becomes a first profile named after the person logged in.
    pub fn load(base: &Paths) -> Self {
        let index = std::fs::read_to_string(Self::index_path(base))
            .ok()
            .and_then(|t| serde_json::from_str::<Index>(&t).ok())
            .unwrap_or_default();
        let mut me = Self { base: base.clone(), index };
        let legacy = base.root.join("progress.json");
        if me.index.profiles.is_empty()
            && legacy.exists()
            && let Ok(p) = me.create(&system_name())
        {
            let dest = me.paths_for(&p).progress();
            if std::fs::rename(&legacy, &dest).is_err() {
                let _ = std::fs::copy(&legacy, &dest);
            }
            me.index.last = Some(p.id);
            let _ = me.save();
        }
        me
    }

    fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.base.root)?;
        let text = serde_json::to_string_pretty(&self.index).map_err(std::io::Error::other)?;
        let path = Self::index_path(&self.base);
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)
    }

    pub fn list(&self) -> &[Profile] {
        &self.index.profiles
    }

    pub fn find(&self, id_or_name: &str) -> Option<&Profile> {
        let q = id_or_name.to_lowercase();
        self.index.profiles.iter().find(|p| p.id == q || p.name.to_lowercase() == q)
    }

    pub fn last(&self) -> Option<&Profile> {
        self.index.last.as_deref().and_then(|id| self.index.profiles.iter().find(|p| p.id == id))
    }

    pub fn create(&mut self, name: &str) -> std::io::Result<Profile> {
        let name = name.trim();
        if name.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "a profile needs a name"));
        }
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while id == GUEST || self.index.profiles.iter().any(|p| p.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        let profile = Profile {
            id,
            name: name.to_string(),
            color: (self.index.profiles.len() % 7) as u8,
            created_at: poddojo_core::now(),
        };
        std::fs::create_dir_all(self.paths_for(&profile).profile_dir())?;
        self.index.profiles.push(profile.clone());
        self.save()?;
        Ok(profile)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> std::io::Result<()> {
        if let Some(p) = self.index.profiles.iter_mut().find(|p| p.id == id)
            && !name.trim().is_empty()
        {
            p.name = name.trim().to_string();
        }
        self.save()
    }

    /// Remove a profile from the list. Its folder is kept, renamed, so nothing is lost by mistake.
    pub fn remove(&mut self, id: &str) -> std::io::Result<()> {
        let dir = self.base.root.join("profiles").join(id);
        if dir.exists() {
            let _ = std::fs::rename(
                &dir,
                self.base.root.join("profiles").join(format!(".removed-{id}-{}", poddojo_core::now())),
            );
        }
        self.index.profiles.retain(|p| p.id != id);
        if self.index.last.as_deref() == Some(id) {
            self.index.last = None;
        }
        self.save()
    }

    /// Remember `id` as the profile to suggest next time (not for the guest).
    pub fn set_last(&mut self, id: &str) -> std::io::Result<()> {
        if id != GUEST {
            self.index.last = Some(id.to_string());
        }
        self.save()
    }

    /// Paths that keep `profile`'s progress in its own folder.
    pub fn paths_for(&self, profile: &Profile) -> Paths {
        Paths { root: self.base.root.clone(), profile: Some(profile.id.clone()) }
    }

    /// A clean guest session: the previous guest's progress is wiped.
    pub fn start_guest(&self) -> Paths {
        let paths = self.paths_for(&Profile::guest());
        let _ = std::fs::remove_dir_all(paths.profile_dir());
        let _ = std::fs::create_dir_all(paths.profile_dir());
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> Paths {
        let dir = std::env::temp_dir().join(format!("poddojo-profiles-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Paths::at(dir)
    }

    #[test]
    fn create_find_and_last() {
        let base = tmp("create");
        let mut p = Profiles::load(&base);
        let a = p.create("David Bujosa").unwrap();
        let b = p.create("David Bujosa").unwrap();
        assert_eq!(a.id, "david-bujosa");
        assert_eq!(b.id, "david-bujosa-2");
        assert_eq!(a.initials(), "DB");
        p.set_last(&b.id).unwrap();
        let again = Profiles::load(&base);
        assert_eq!(again.last().map(|x| x.id.as_str()), Some("david-bujosa-2"));
        assert_eq!(again.find("David Bujosa").map(|x| x.id.as_str()), Some("david-bujosa"));
        assert!(p.create("  ").is_err());
        assert_ne!(p.create("Guest").unwrap().id, GUEST);
        let _ = std::fs::remove_dir_all(&base.root);
    }

    #[test]
    fn legacy_progress_is_migrated() {
        let base = tmp("legacy");
        std::fs::create_dir_all(&base.root).unwrap();
        std::fs::write(base.root.join("progress.json"), "{\"version\":1}").unwrap();
        let p = Profiles::load(&base);
        let me = p.last().unwrap().clone();
        assert!(p.paths_for(&me).progress().exists());
        assert!(!base.root.join("progress.json").exists());
        let _ = std::fs::remove_dir_all(&base.root);
    }

    #[test]
    fn guest_starts_clean_and_removed_profiles_are_kept_on_disk() {
        let base = tmp("guest");
        let mut p = Profiles::load(&base);
        let g = p.start_guest();
        std::fs::write(g.progress(), "{}").unwrap();
        assert!(!p.start_guest().progress().exists());
        let a = p.create("Ana").unwrap();
        std::fs::write(p.paths_for(&a).progress(), "{}").unwrap();
        p.remove(&a.id).unwrap();
        assert!(p.list().is_empty());
        let kept = std::fs::read_dir(base.root.join("profiles"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with(".removed-ana"));
        assert!(kept);
        let _ = std::fs::remove_dir_all(&base.root);
    }
}
