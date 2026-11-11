//! The player's position, through the variables Wand's maps use.

use anyhow::{Context, Result, ensure};
use futures::future;

use super::{Outcome, Session};

const AXES: [&str; 3] = ["wm_coord_x", "wm_coord_y", "wm_coord_z"];

impl Session {
    /// In the game's own coordinates.
    pub async fn position(&self) -> Result<[f64; 3]> {
        let _turn = self.coords.lock().await;
        self.signal("wm_coord_get").await?;
        let values = self.shared.values.lock().unwrap();
        let mut position = [0.; 3];
        for (axis, name) in position.iter_mut().zip(AXES) {
            *axis = *values.get(name).with_context(|| format!("the trainer sent no {name}"))?;
        }
        Ok(position)
    }

    pub async fn teleport(&self, to: [f64; 3]) -> Result<()> {
        let _turn = self.coords.lock().await;
        let sets = AXES.iter().zip(to).map(|(name, value)| self.set_value(name, value));
        let outcomes = future::try_join_all(sets).await?;
        ensure!(outcomes.iter().all(|outcome| *outcome == Outcome::Set), "the trainer refused the position");
        self.signal("wm_coord_set").await
    }

    /// Sets a request variable to -1: the trainer acts on it, then answers by setting it to 1.
    async fn signal(&self, name: &str) -> Result<()> {
        // So an earlier answer does not count for this request.
        self.shared.values.lock().unwrap().insert(name.to_owned(), -1.);
        let outcome = self.set_value(name, -1.).await?;
        let answered = self.shared.values.lock().unwrap().get(name) == Some(&1.);
        ensure!(outcome == Outcome::Changed && answered, "the trainer did not act on {name}");
        Ok(())
    }
}
