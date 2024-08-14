use tokio::sync::{mpsc, oneshot};
use surrealdb::sql::Thing;
use anyhow::anyhow;
use std::collections::HashMap;
use crate::database_manager::{DatabaseRequest, Responder};

pub enum WagerRequest {
    ResolveWager {
        wager_id: Thing,
        winning_option: Thing,
        responder: Responder<()>,
    },
}

pub struct WagerManager {
    work_queue: mpsc::Receiver<WagerRequest>,
    database_requester: mpsc::Sender<DatabaseRequest>,
}

//NOTE: No functions in this impl may crash
impl WagerManager {
    pub fn new(
        work_queue: mpsc::Receiver<WagerRequest>,
        database_requester: mpsc::Sender<DatabaseRequest>,
    ) -> Self {
        Self {
            work_queue,
            database_requester,
        }
    }

    pub async fn manage(&mut self) {
        while let Some(request) = self.work_queue.recv().await {
            match request {
                WagerRequest::ResolveWager {
                    wager_id,
                    winning_option,
                    responder,
                } => {
                    // we do not care if the receiver has already disappeared
                    responder
                        .send(self.resolve_wager(wager_id, winning_option).await)
                        .ok();
                }
            }
        }
    }

    async fn resolve_wager(&mut self, wager_id: Thing, winning_option_id: Thing) -> anyhow::Result<()> {
        let (wager_tx, wager_rx) = oneshot::channel();
        self.database_requester
            .send(DatabaseRequest::GetWagerInfo {
                id: wager_id,
                responder: wager_tx,
            })
            .await?;

        let wager_info = wager_rx.await??.ok_or(anyhow!("invalid wager"))?;
        let mut wager_total_map = HashMap::new();
        for option in &wager_info.options {
            let total: u64 = option.bets.iter().map(|bet| bet.val).sum();
            wager_total_map.insert(option.id.clone(), total);
        }

        let abs_total: u64 =
            wager_total_map.iter().map(|(_, value)| value).sum::<u64>() + wager_info.pot;
        let winning_ratio = abs_total as f64
            / *wager_total_map
                .get(&winning_option_id.id.to_string())
                .ok_or(anyhow::Error::msg("invalid wager totals"))? as f64;

        let winning_bets = &wager_info
            .options
            .iter()
            .find(|option| option.id.clone() == winning_option_id.id.to_string())
            .ok_or(anyhow::Error::msg("no winning bets"))?
            .bets;

        for winning_bet in winning_bets {
            let (payout_tx, payout_rx) = oneshot::channel();
            self.database_requester
                .send(DatabaseRequest::ProvidePayout {
                    bet_info: winning_bet.clone(),
                    winning_ratio,
                    responder: payout_tx,
                })
                .await?;
            payout_rx.await??;
        }
        Ok(())
    }
}
