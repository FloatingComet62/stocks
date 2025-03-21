// Main thing to do now is for agents to hold long for certain companies

use rand::{random, Rng};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use stocks::{
    entities::{
        agents::{Agent, Agents},
        companies::{Companies, Company},
    },
    load, log,
    logger::Log,
    market::Market,
    max,
    trade_house::{FailedOffer, StockOption, Trade},
    transaction::TodoTransaction,
    SimulationError, AGENTS_DATA_FILENAME, COMPANIES_DATA_FILENAME, MIN_STRIKE_PRICE,
    NUM_OF_AGENTS, NUM_OF_COMPANIES,
};

fn spend_function(x: f64) -> f64 {
    // went off feeling
    0.99 * (1.0 - (-0.01 * x * x).exp()) + 0.01
}

fn rand_spend_portion_wealth(rng: &mut impl Rng) -> f64 {
    let Ok(normal) = Normal::new(0.0, 1.0) else {
        // If the normal distribution fails, fuck it then
        return 0.01;
    };
    spend_function(normal.sample(rng))
}

fn main() {
    let seed = random();
    if let Err(e) = Log::new().to_file(&format!("Seed: {}\n", seed)) {
        log!(warn "Failed to save seed to the file\n{:?}", e)
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    log!(info "Loading local file data");
    let agent_file = load::<Vec<Agent>>(AGENTS_DATA_FILENAME);
    let company_file = load::<Vec<Company>>(COMPANIES_DATA_FILENAME);

    let mut companies = match company_file {
        Ok(company_data) => {
            log!(info "Loaded companies");
            Companies::load(company_data.as_slice())
        }
        Err(ref e) => {
            log!(warn "Company file not found\n{:?}", e);
            Companies::rand(NUM_OF_COMPANIES as usize, 0, &mut rng)
        }
    };

    let mut agents = match agent_file {
        Ok(agent_data) => {
            log!(info "Loaded agents");
            Agents::load(agent_data.as_slice())
        }
        Err(ref e) => {
            log!(warn "Agents file not found\n{:?}", e);
            let mut a = Agents::new();
            let rng1 = ChaCha8Rng::seed_from_u64(seed + 1);
            let rng2 = ChaCha8Rng::seed_from_u64(seed + 2);
            let rng3 = ChaCha8Rng::seed_from_u64(seed + 3);
            a.rand_introduce_new_agents(rng1, rng2, NUM_OF_AGENTS, companies.num_of_companies)
                .unwrap();
            a.rand_give_preferences(rng3, companies.num_of_companies)
                .unwrap();
            a
        }
    };

    let mut market = Market::new();

    let mut expired_trades: HashMap<u64, Vec<FailedOffer<Trade>>> = HashMap::new();
    let mut expired_options: HashMap<u64, Vec<FailedOffer<StockOption>>> = HashMap::new();

    let mut todo_transactions: Vec<TodoTransaction> = Vec::new();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let mut i: i128 = 0;
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
    while running.load(Ordering::SeqCst) {
        i += 1;
        agents.try_offers.clear();
        println!("{}", i);
        if i % 5 == 0 {
            for company_id in companies.iter() {
                let Some(market_value) = companies.market_values.get_mut(company_id as usize)
                else {
                    continue;
                };
                market.tick_individual_company(company_id, market_value);
            }
            market.tick_failures(&mut expired_trades, &mut expired_options);
        }
        if i % 20 == 0 {
            companies.rand_release_news(&mut agents, &mut rng);
        }
        agents
            .alert_agents(&expired_trades, &expired_options)
            .unwrap();
        expired_trades.clear();
        expired_options.clear();

        for agent_id in agents.iter() {
            let (company_id, mut action) = agents
                .preferences
                .get_preferred_random(agent_id, &mut rng)
                .unwrap();

            // small portion of people who sell low and buy high, because .... IDK WHY
            if rng.gen_ratio(5, 100) {
                action = action.complement();
            }

            let failable_value = rng.gen_range(10.0..2_000.0);
            let current_price = companies
                .get_current_price(company_id)
                .unwrap_or(failable_value);
            companies.market_values[company_id as usize].current_price = current_price;
            let strike_price = max(MIN_STRIKE_PRICE, current_price + rng.gen_range(-10.0..10.0));
            let want_to_spend =
                agents.balances.get(agent_id).unwrap() * rand_spend_portion_wealth(&mut rng);
            let rough_amount_of_stocks = (want_to_spend / strike_price).floor() as u64;
            if rough_amount_of_stocks == 0 {
                // bruh, just don't trade anything
                continue;
            }

            todo_transactions.push(TodoTransaction {
                agent_id,
                company_id,
                strike_price,
                action,
                trade: Trade::new(rough_amount_of_stocks),
            });
        }
        let news_probability_distribution = &companies.generate_preferences_from_news(&mut rng);
        agents.rand_give_preferences_from_news(&mut rng, news_probability_distribution);
        let Err(e) = market.rand_do_trade(
            &mut rng,
            &mut agents,
            &mut companies,
            &mut todo_transactions,
        ) else {
            todo_transactions.clear();
            continue;
        };
        todo_transactions.clear();
        match e {
            SimulationError::AgentNotFound(agent_id) => {
                log!(warn "Agent not found: {}", agent_id);
            }
            SimulationError::NoData => {
                log!(warn "No data");
            }
            SimulationError::Unspendable | SimulationError::UnDoable => {
                continue;
            }
        }
    }
    log!(info "Exiting at index {:?}", i);
    log!(info "Saving data");

    /*
    if let Err(e) = save(agents.save().unwrap(), AGENTS_DATA_FILENAME) {
        log!(warn "Failed to save agents data\n{:?}", e);
    } else {
        log!(info "Saved agents");
    }
    if let Err(e) = save(companies.save(), COMPANIES_DATA_FILENAME) {
        log!(warn "Failed to save company data\n{:?}", e);
    } else {
        log!(info "Saved companies");
    }
    */
    log!(info "Exit");
}
