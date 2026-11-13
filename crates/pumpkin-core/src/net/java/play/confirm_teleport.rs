#[allow(clippy::wildcard_imports)]
use super::*;

/// What to do with a teleport confirmation, following vanilla's
/// `ServerGamePacketListenerImpl.handleAcceptTeleportPacket`.
#[derive(Debug, PartialEq, Eq)]
enum ConfirmOutcome {
    /// It confirms the teleport we're waiting for.
    Accept,
    /// It confirms our latest teleport, but we aren't waiting for it any more:
    /// a duplicate. Vanilla disconnects for invalid player movement.
    Disconnect,
    /// It confirms an older teleport. Vanilla ignores it; the client sends a
    /// confirmation for every teleport in order, so a newer one is coming.
    Ignore,
}

/// `awaiting` is the id of the teleport we're waiting for, if any; `latest` is
/// the id of the last teleport we sent.
const fn confirm_outcome(awaiting: Option<i32>, latest: i32, confirmed: i32) -> ConfirmOutcome {
    match awaiting {
        Some(id) if id == confirmed => ConfirmOutcome::Accept,
        None if confirmed == latest => ConfirmOutcome::Disconnect,
        _ => ConfirmOutcome::Ignore,
    }
}

impl JavaClient {
    pub fn handle_confirm_teleport(&self, player: &Player, confirm_teleport: &SConfirmTeleport) {
        let outcome = {
            let mut awaiting_teleport = player
                .awaiting_teleport
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let latest = player.teleport_id_count.load(Ordering::Relaxed);
            let outcome = confirm_outcome(
                awaiting_teleport.as_ref().map(|(id, _)| id.0),
                latest,
                confirm_teleport.teleport_id.0,
            );
            if outcome == ConfirmOutcome::Accept
                && let Some((_, position)) = awaiting_teleport.take()
            {
                // We should set the position now to what we requested in the teleport packet.
                // This may fix issues when the client sends the position while being teleported.
                player.get_entity().set_pos(position);
            }
            outcome
        };

        if outcome == ConfirmOutcome::Disconnect {
            self.try_kick(&TextComponent::translate(
                "multiplayer.disconnect.invalid_player_movement",
                [],
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfirmOutcome, confirm_outcome};

    #[test]
    fn accepts_the_awaited_teleport() {
        assert_eq!(confirm_outcome(Some(3), 3, 3), ConfirmOutcome::Accept);
    }

    #[test]
    fn ignores_an_older_teleport_while_a_newer_one_is_awaited() {
        // Two teleports in a row: the client confirms 2 before 3.
        assert_eq!(confirm_outcome(Some(3), 3, 2), ConfirmOutcome::Ignore);
    }

    #[test]
    fn ignores_an_older_teleport_once_nothing_is_awaited() {
        assert_eq!(confirm_outcome(None, 3, 2), ConfirmOutcome::Ignore);
    }

    #[test]
    fn disconnects_on_a_duplicate_confirmation() {
        assert_eq!(confirm_outcome(None, 3, 3), ConfirmOutcome::Disconnect);
    }
}
