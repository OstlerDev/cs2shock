use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use log::{debug, info};
use rand::{rngs::StdRng, thread_rng, Rng, SeedableRng};
use tokio::sync::{Mutex, RwLock};

use crate::{
    config::{self, shock_duration_to_tenths, Config},
    gamestateintegration::{MapPhase, Payload, RoundPhase},
    pishock,
    setup::EXPECTED_GSI_URI,
    AppState, GameState, PendingShock, PlayerState,
};

pub async fn run(config: Arc<RwLock<Config>>) {
    let state = AppState {
        game_state: Arc::from(Mutex::from(GameState::default())),
        config: config.clone(),
    };

    let app = Router::new()
        .route("/data", post(read_data))
        .with_state(state);

    info!("Starting server on {}", EXPECTED_GSI_URI);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn scale_last_hit_value(health_before_death: i32, max_value: i32) -> i32 {
    let health_ratio = health_before_death.clamp(1, 100) as f32 / 100.0;
    let scaled = (health_ratio * max_value as f32).ceil() as i32;

    scaled.clamp(1, max_value)
}

fn warning_beep_delay(config: &Config) -> Option<Duration> {
    config
        .warning_beep_before_shock
        .then(|| Duration::from_secs(config.warning_beep_duration as u64))
}

fn should_send_shock_after_roll(shock_chance: i32, roll: i32) -> bool {
    roll <= shock_chance
}

fn should_send_shock(config: &Config) -> bool {
    if config.shock_chance >= 100 {
        return true;
    }

    if config.shock_chance <= 0 {
        return false;
    }

    let roll = thread_rng().gen_range(1..=100);
    should_send_shock_after_roll(config.shock_chance, roll)
}

fn should_trigger_death_sequence(
    previous_deaths: i32,
    current_deaths: i32,
    triggered_this_round: bool,
    shocks_disabled_until_next_round: bool,
    round_phase: &RoundPhase,
) -> bool {
    current_deaths > previous_deaths
        && !triggered_this_round
        && !shocks_disabled_until_next_round
        && *round_phase == RoundPhase::Live
}

fn should_prevent_shock_for_round_kills(config: &Config, round_kills: i32) -> bool {
    config.prevent_shock_if_round_kills_reached
        && round_kills >= config.round_kills_to_prevent_shock
}

fn normalize_team_name(team: &str) -> String {
    team.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn did_player_win_round(player_team: Option<&str>, round_winner: Option<&str>) -> bool {
    let Some(player_team) = player_team else {
        return false;
    };
    let Some(round_winner) = round_winner else {
        return false;
    };

    let normalized_player_team = normalize_team_name(player_team);
    let normalized_round_winner = normalize_team_name(round_winner);

    normalized_player_team == normalized_round_winner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoundOutcome {
    Won,
    Lost,
    Unknown,
}

fn round_outcome_for_player(player_team: Option<&str>, round_winner: Option<&str>) -> RoundOutcome {
    let Some(player_team) = player_team else {
        return RoundOutcome::Unknown;
    };
    let Some(round_winner) = round_winner else {
        return RoundOutcome::Unknown;
    };

    if did_player_win_round(Some(player_team), Some(round_winner)) {
        RoundOutcome::Won
    } else {
        RoundOutcome::Lost
    }
}

fn scale_last_hit_duration_ms(health_before_death: i32, max_duration: f32) -> u64 {
    let duration_tenths =
        scale_last_hit_value(health_before_death, shock_duration_to_tenths(max_duration));
    duration_tenths as u64 * 100
}

fn calculate_shock_values(config: &Config, health_before_death: i32) -> (i32, u64) {
    match config.shock_mode {
        config::ShockMode::Random => {
            let mut rng = StdRng::from_entropy();
            let intensity = rng.gen_range(config.min_intensity..=config.max_intensity);
            let duration_tenths = rng.gen_range(
                shock_duration_to_tenths(config.min_duration)
                    ..=shock_duration_to_tenths(config.max_duration),
            );
            (intensity, duration_tenths as u64 * 100)
        }
        config::ShockMode::LastHitPercentage => {
            let intensity = scale_last_hit_value(health_before_death, config.max_intensity);
            let duration = scale_last_hit_duration_ms(health_before_death, config.max_duration);
            (intensity, duration)
        }
    }
}

async fn send_shock_sequence(
    config_handle: Arc<RwLock<Config>>,
    config: &Config,
    intensity: i32,
    duration_ms: u64,
) {
    if let Some(delay) = warning_beep_delay(config) {
        info!(
            "Sending warning beep before shock (duration: {})",
            config.warning_beep_duration
        );
        pishock::beep(config_handle.clone(), config.warning_beep_duration).await;
        tokio::time::sleep(delay).await;
    }

    if !should_send_shock(config) {
        info!(
            "Skipping shock because the shock chance roll failed (shock chance: {}%)",
            config.shock_chance
        );
        return;
    }

    pishock::shock(config_handle, intensity, duration_ms).await;
}

async fn read_data(State(state): State<AppState>, Json(payload): Json<Payload>) -> StatusCode {
    let mut game_state = state.game_state.lock().await;
    let config = state.config.read().await.clone();
    let previous_round_phase = game_state.round_phase.clone();
    let mut should_beep_on_match_start = false;
    let mut should_beep_on_round_start = false;
    let mut pending_shock = None;

    if let Some(provider) = payload.provider {
        game_state.steam_id = provider.steamid;
    }

    if let Some(map) = payload.map {
        if game_state.map_phase == MapPhase::Warmup && map.phase == MapPhase::Live {
            info!("Match started");

            if config.beep_on_match_start {
                should_beep_on_match_start = true;
            }

            // Reset game state to default
            game_state.reset();
        }

        game_state.map_phase = map.phase;
    }

    if let Some(round) = payload.round.as_ref() {
        if game_state.round_phase != round.phase && round.phase == RoundPhase::Freezetime {
            game_state.triggered_this_round = false;
            game_state.shocks_disabled_until_next_round = false;
            if game_state.pending_round_loss_shock.take().is_some() {
                debug!("Cleared deferred round-loss shock at freezetime");
            }
        }

        if game_state.round_phase == RoundPhase::Freezetime && round.phase == RoundPhase::Live {
            game_state.triggered_this_round = false;
            game_state.shocks_disabled_until_next_round = false;
            if game_state.pending_round_loss_shock.take().is_some() {
                debug!("Cleared deferred round-loss shock at round start");
            }

            if config.beep_on_round_start {
                info!("Round started");
                should_beep_on_round_start = true;
            }
        }

        game_state.round_phase = round.phase.clone();
    }

    if game_state.map_phase == MapPhase::Live {
        if let Some(player) = payload.player {
            if player.steamid == game_state.steam_id {
                game_state.player_team = player.team.clone();
                let already_triggered_this_round = game_state.triggered_this_round;
                let shocks_disabled_until_next_round = game_state.shocks_disabled_until_next_round;
                let round_phase = if previous_round_phase == RoundPhase::Live
                    || game_state.round_phase == RoundPhase::Live
                {
                    RoundPhase::Live
                } else {
                    game_state.round_phase.clone()
                };

                if let Some(player_state) = &mut game_state.player_state {
                    let mut should_mark_triggered_this_round = false;
                    let mut deferred_round_loss_shock = None;

                    if player_state.health > player.state.health && player.state.health > 0 {
                        // Took damage and survived

                        /*
                        println!("Player took damage, vibrating");

                        let diff = player_state.health - player.state.health;

                        let res = pishock::post(
                            &config,
                            pishock::PiShockOp::Vibrate {
                                intensity: diff,
                                duration: 1,
                            },
                        )
                        .await;

                        match res {
                            Ok(code) => println!("Vibrated with code {}", code),
                            Err(e) => println!("Error while vibrating: {}", e),
                        };
                         */
                    }

                    if should_trigger_death_sequence(
                        player_state.deaths,
                        player.match_stats.deaths,
                        already_triggered_this_round,
                        shocks_disabled_until_next_round,
                        &round_phase,
                    ) {
                        should_mark_triggered_this_round = true;

                        if should_prevent_shock_for_round_kills(&config, player.state.round_kills) {
                            info!(
                                "Player died after reaching round kill threshold ({}), skipping shock",
                                player.state.round_kills
                            );
                        } else {
                            let (intensity, duration) =
                                calculate_shock_values(&config, player_state.health);

                            if config.shock_on_round_loss_only {
                                info!("Player died, deferring shock until round result");
                                deferred_round_loss_shock = Some(PendingShock {
                                    intensity,
                                    duration_ms: duration,
                                });
                            } else {
                                info!("Player died, shocking");
                                pending_shock = Some((intensity, duration));
                            }
                        }
                    }

                    player_state.health = player.state.health;
                    player_state.armor = player.state.armor;
                    player_state.kills = player.match_stats.kills;
                    player_state.deaths = player.match_stats.deaths;

                    if should_mark_triggered_this_round {
                        game_state.triggered_this_round = true;
                    }

                    if let Some(deferred_shock) = deferred_round_loss_shock {
                        game_state.pending_round_loss_shock = Some(deferred_shock);
                    }
                } else {
                    println!("Player state initialized");

                    game_state.player_state = Some(PlayerState {
                        health: player.state.health,
                        armor: player.state.armor,
                        kills: player.match_stats.kills,
                        deaths: player.match_stats.deaths,
                    });
                }
            }
        }
    }

    if game_state.round_phase == RoundPhase::Over {
        let player_team = game_state.player_team.clone();
        let round_winner = payload
            .round
            .as_ref()
            .and_then(|round| round.win_team.clone());
        let round_outcome =
            round_outcome_for_player(player_team.as_deref(), round_winner.as_deref());

        debug!(
            "Round over result: player_team={:?}, round_winner={:?}, outcome={:?}",
            player_team, round_winner, round_outcome
        );

        if round_outcome == RoundOutcome::Won {
            game_state.shocks_disabled_until_next_round = true;
            debug!(
                "Suppressed shocks until next round: player_team={:?}, round_winner={:?}",
                player_team, round_winner
            );
            info!("Round won, disabling shocks until next round");

            if game_state.pending_round_loss_shock.take().is_some() {
                info!("Cancelled deferred shock because your team won the round");
            }
        } else if round_outcome == RoundOutcome::Lost {
            if let Some(deferred_shock) = game_state.pending_round_loss_shock.take() {
                info!("Round lost after death, triggering deferred shock");
                pending_shock = Some((deferred_shock.intensity, deferred_shock.duration_ms));
                game_state.shocks_disabled_until_next_round = true;
            }
        } else if game_state.pending_round_loss_shock.take().is_some() {
            debug!(
                "Cleared deferred shock because round outcome was unknown: player_team={:?}, round_winner={:?}",
                player_team, round_winner
            );
        }
    }

    drop(game_state);

    if should_beep_on_match_start {
        pishock::beep(state.config.clone(), 2).await;
    }

    if should_beep_on_round_start {
        pishock::beep(state.config.clone(), 1).await;
    }

    if let Some((intensity, duration)) = pending_shock {
        send_shock_sequence(state.config.clone(), &config, intensity, duration).await;
    }

    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_shock_values, did_player_win_round, round_outcome_for_player,
        scale_last_hit_value, should_prevent_shock_for_round_kills, should_send_shock,
        should_send_shock_after_roll, should_trigger_death_sequence, warning_beep_delay,
        RoundOutcome,
    };
    use crate::config::{Config, ShockMode};
    use crate::gamestateintegration::RoundPhase;
    use std::time::Duration;

    #[test]
    fn scale_last_hit_value_rounds_up_low_non_zero_values() {
        assert_eq!(scale_last_hit_value(1, 15), 1);
    }

    #[test]
    fn scale_last_hit_value_uses_percentage_of_maximum() {
        assert_eq!(scale_last_hit_value(25, 83), 21);
    }

    #[test]
    fn warning_beep_delay_uses_default_duration() {
        assert_eq!(
            warning_beep_delay(&Config::default()),
            Some(Duration::from_secs(2))
        );
    }

    #[test]
    fn warning_beep_delay_uses_configured_duration() {
        let mut config = Config::default();
        config.warning_beep_before_shock = true;
        config.warning_beep_duration = 3;

        assert_eq!(warning_beep_delay(&config), Some(Duration::from_secs(3)));
    }

    #[test]
    fn should_send_shock_after_roll_respects_probability() {
        assert!(should_send_shock_after_roll(50, 50));
        assert!(!should_send_shock_after_roll(50, 51));
    }

    #[test]
    fn should_send_shock_respects_zero_percent_chance_without_warning_beep() {
        let mut config = Config::default();
        config.warning_beep_before_shock = false;
        config.shock_chance = 0;

        assert!(!should_send_shock(&config));
    }

    #[test]
    fn should_send_shock_respects_hundred_percent_chance_without_warning_beep() {
        let mut config = Config::default();
        config.warning_beep_before_shock = false;
        config.shock_chance = 100;

        assert!(should_send_shock(&config));
    }

    #[test]
    fn should_send_shock_respects_zero_percent_chance_with_warning_beep() {
        let mut config = Config::default();
        config.warning_beep_before_shock = true;
        config.shock_chance = 0;

        assert!(!should_send_shock(&config));
    }

    #[test]
    fn should_send_shock_respects_hundred_percent_chance_with_warning_beep() {
        let mut config = Config::default();
        config.warning_beep_before_shock = true;
        config.shock_chance = 100;

        assert!(should_send_shock(&config));
    }

    #[test]
    fn should_trigger_death_sequence_only_once_per_round() {
        assert!(should_trigger_death_sequence(
            3,
            4,
            false,
            false,
            &RoundPhase::Live
        ));
        assert!(!should_trigger_death_sequence(
            3,
            4,
            true,
            false,
            &RoundPhase::Live
        ));
        assert!(!should_trigger_death_sequence(
            3,
            3,
            false,
            false,
            &RoundPhase::Live
        ));
    }

    #[test]
    fn should_not_trigger_death_sequence_after_round_win() {
        assert!(!should_trigger_death_sequence(
            3,
            4,
            false,
            true,
            &RoundPhase::Live
        ));
        assert!(!should_trigger_death_sequence(
            3,
            4,
            false,
            false,
            &RoundPhase::Over
        ));
    }

    #[test]
    fn round_kill_threshold_suppression_uses_default_threshold() {
        assert!(!should_prevent_shock_for_round_kills(&Config::default(), 0));
        assert!(should_prevent_shock_for_round_kills(&Config::default(), 1));
    }

    #[test]
    fn round_kill_threshold_suppression_waits_until_threshold_is_reached() {
        let mut config = Config::default();
        config.prevent_shock_if_round_kills_reached = true;
        config.round_kills_to_prevent_shock = 3;

        assert!(!should_prevent_shock_for_round_kills(&config, 2));
    }

    #[test]
    fn round_kill_threshold_suppression_triggers_at_threshold() {
        let mut config = Config::default();
        config.prevent_shock_if_round_kills_reached = true;
        config.round_kills_to_prevent_shock = 3;

        assert!(should_prevent_shock_for_round_kills(&config, 3));
    }

    #[test]
    fn round_kill_threshold_suppression_triggers_above_threshold() {
        let mut config = Config::default();
        config.prevent_shock_if_round_kills_reached = true;
        config.round_kills_to_prevent_shock = 3;

        assert!(should_prevent_shock_for_round_kills(&config, 4));
    }

    #[test]
    fn did_player_win_round_matches_team_names() {
        assert!(did_player_win_round(Some("CT"), Some("CT")));
        assert!(did_player_win_round(
            Some("Counter-Terrorist"),
            Some("counter_terrorist")
        ));
        assert!(!did_player_win_round(Some("T"), Some("CT")));
    }

    #[test]
    fn round_outcome_for_player_detects_win_loss_and_unknown() {
        assert_eq!(
            round_outcome_for_player(Some("CT"), Some("CT")),
            RoundOutcome::Won
        );
        assert_eq!(
            round_outcome_for_player(Some("T"), Some("CT")),
            RoundOutcome::Lost
        );
        assert_eq!(
            round_outcome_for_player(None, Some("CT")),
            RoundOutcome::Unknown
        );
    }

    #[test]
    fn calculate_shock_values_uses_last_hit_percentage_mode() {
        let mut config = Config::default();
        config.shock_mode = ShockMode::LastHitPercentage;
        config.max_intensity = 80;
        config.max_duration = 1.2;

        assert_eq!(calculate_shock_values(&config, 25), (20, 300));
    }
}
