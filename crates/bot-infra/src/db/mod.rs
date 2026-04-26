use anyhow::Result;
use bot_core::{can_transition, LegSide, MarketCycleId, TradeState};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{pool::PoolConnection, postgres::PgPoolOptions, PgPool, Postgres, Row};
use std::collections::HashMap;
use uuid::Uuid;

mod auto_claim;
mod core;
mod mappers;
mod models;
mod orders;
mod risk;
mod runs;
mod trade_builder;
mod trade_flow;
mod trade_flow_overlap;
mod trade_flow_runtime_snapshots;
mod trade_flow_steps;
mod trades;

pub use core::{PostgresRepository, RunnerSingletonDbLock};
pub(crate) use mappers::{db_to_leg_side, leg_side_to_db, parse_state};
pub use models::*;
