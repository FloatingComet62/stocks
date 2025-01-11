use crate::{
    entities::{companies::Companies, Balances},
    market::{ActionState, Market},
    trade_house::{FailedOffer, StockOption, Trade, TradeAction},
    transaction::{TodoTransactions, Transaction},
    SimulationError, NUM_OF_AGENTS, TIMELINE_SIZE_LIMIT,
};
use rand::{random, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn combine(a: u64, b: u64) -> u128 {
    (a as u128) << 64 | b as u128
}

fn get_first(a: u128) -> u64 {
    (a >> 64) as u64
}

fn get_second(a: u128) -> u64 {
    (a & 0xFFFFFFFFFFFFFFFF) as u64
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AgentHoldings(pub HashMap<u64, u64>);

#[derive(Debug, Clone, Default)]
pub struct Holdings(HashMap<u128, u64>);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub data: Vec<(u64, TradeAction)>,
    pub target_index: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPreferences(Timeline);

#[derive(Debug, Clone, Default)]
pub struct Preferences(pub Vec<Timeline>);

#[derive(Default)]
pub struct Agents {
    pub num_of_agents: u64,
    pub holdings: Holdings,
    pub balances: Balances,
    pub preferences: Preferences,
    pub try_offers: HashMap<u128, f64>,
}

#[derive(Serialize, Deserialize)]
pub struct Agent {
    pub id: u64,
    pub balance: f64,
    pub holding: AgentHoldings,
    pub preferences: AgentPreferences,
}

impl Holdings {
    pub fn insert(&mut self, agent_id: u64, company_id: u64, number_of_shares: u64) {
        self.0
            .insert(combine(agent_id, company_id), number_of_shares);
    }
    pub fn get(&self, agent_id: u64, company_id: u64) -> u64 {
        match self.0.get(&combine(agent_id, company_id)) {
            Some(share_count) => *share_count,
            None => 0,
        }
    }
    pub fn get_u128(&self, id: u128) -> u64 {
        match self.0.get(&id) {
            Some(share_count) => *share_count,
            None => 0,
        }
    }
    pub fn push_from_txn(&mut self, target_agent_id: u64, transaction: &Transaction) {
        let company = self
            .0
            .get_mut(&combine(target_agent_id, transaction.company_id));
        match company {
            Some(share_count) => {
                *share_count += transaction.number_of_shares;
            }
            None => {
                self.0.insert(
                    combine(target_agent_id, transaction.company_id),
                    transaction.number_of_shares,
                );
            }
        }
    }
    pub fn pop_from_txn(
        &mut self,
        target_agent_id: u64,
        transaction: &Transaction,
    ) -> Result<(), SimulationError> {
        let Some(share_count) = self
            .0
            .get_mut(&combine(target_agent_id, transaction.company_id))
        else {
            return Err(SimulationError::Unspendable);
        };
        if *share_count < transaction.number_of_shares {
            return Err(SimulationError::Unspendable);
        }
        *share_count -= transaction.number_of_shares;
        Ok(())
    }
    pub fn push(&mut self, agent_id: u64, company_id: u64, number_of_shares: u64) {
        let Some(share_count) = self.0.get_mut(&combine(agent_id, company_id)) else {
            self.0
                .insert(combine(agent_id, company_id), number_of_shares);
            return;
        };
        *share_count += number_of_shares;
    }
    pub fn pop(
        &mut self,
        agent_id: u64,
        company_id: u64,
        number_of_shares: u64,
    ) -> Result<(), SimulationError> {
        let Some(share_count) = self.0.get_mut(&combine(agent_id, company_id)) else {
            return Err(SimulationError::Unspendable);
        };
        if *share_count < number_of_shares {
            return Err(SimulationError::Unspendable);
        }

        *share_count -= number_of_shares;
        Ok(())
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            data: vec![],
            target_index: 0,
        }
    }
    pub fn add(&mut self, data: &[(u64, TradeAction)]) {
        if self.data.len() == TIMELINE_SIZE_LIMIT {
            for (i, data_item) in data.iter().enumerate().take(self.data.len()) {
                self.data[(i + self.target_index) % TIMELINE_SIZE_LIMIT] = *data_item;
            }
            return;
        }
        if (data.len() + self.data.len()) <= TIMELINE_SIZE_LIMIT {
            self.data.extend(data.iter());
            return;
        }
        let extend_size = TIMELINE_SIZE_LIMIT - self.data.len();
        let destination_index = data.len() - extend_size;

        self.data.extend(data[0..extend_size].iter());
        self.data[0..destination_index].copy_from_slice(&data[extend_size..]);
        self.target_index = destination_index;
    }
    pub fn get_rng(&self, rng: &mut impl Rng) -> Result<(u64, TradeAction), SimulationError> {
        if self.data.is_empty() {
            return Err(SimulationError::NoData);
        }
        let index = rng.gen_range(0..self.data.len());
        Ok(self.data[index])
    }
    pub fn recency_bias(
        &self,
        bias_size: usize,
        rng: &mut impl Rng,
    ) -> Result<(u64, TradeAction), SimulationError> {
        if bias_size >= self.data.len() {
            return self.get_rng(rng);
        }
        let index = rng.gen_range(0..bias_size);
        if bias_size < self.target_index {
            return Ok(self.data[index + (self.target_index - bias_size)]);
        }
        if index < self.target_index {
            return Ok(self.data[index]);
        }
        Ok(self.data[self.data.len() - (index - self.target_index) - 1])
    }
}

impl Preferences {
    pub fn add(
        &mut self,
        agent_id: u64,
        company_id: u64,
        preference: u64,
    ) -> Result<(), SimulationError> {
        let Some(timeline) = self.0.get_mut(agent_id as usize) else {
            return Err(SimulationError::AgentNotFound(agent_id));
        };
        timeline.add(&vec![(company_id, TradeAction::Buy); preference as usize]);
        Ok(())
    }
    pub fn sub(
        &mut self,
        agent_id: u64,
        company_id: u64,
        preference: u64,
    ) -> Result<(), SimulationError> {
        let Some(timeline) = self.0.get_mut(agent_id as usize) else {
            return Err(SimulationError::AgentNotFound(agent_id));
        };
        timeline.add(&vec![(company_id, TradeAction::Sell); preference as usize]);
        Ok(())
    }
    pub fn get_preferred_random(
        &self,
        agent_id: u64,
        rng: &mut impl Rng,
    ) -> Result<(u64, TradeAction), SimulationError> {
        let Some(agent) = self.0.get(agent_id as usize) else {
            return Err(SimulationError::AgentNotFound(agent_id));
        };
        agent.get_rng(rng)
    }
}

impl Agents {
    pub fn new() -> Self {
        Self {
            num_of_agents: 0,
            balances: Balances(vec![]),
            ..Default::default()
        }
    }
    pub fn load(agents: &[Agent]) -> Self {
        let num_of_agents = agents.len() as u64;
        let mut balances = Vec::with_capacity(agents.len());
        let mut holdings = Holdings::default();
        let mut preferences = Vec::with_capacity(agents.len());
        for agent in agents.iter() {
            balances.push(agent.balance);
            for (company_id, holding) in agent.holding.0.iter() {
                holdings.insert(agent.id, *company_id, *holding);
            }
            preferences.push(agent.preferences.0.clone());
        }
        Self {
            num_of_agents,
            balances: Balances(balances),
            holdings,
            preferences: Preferences(preferences),
            try_offers: HashMap::new(),
        }
    }
    pub fn save(&self) -> Result<Vec<Agent>, SimulationError> {
        let mut agents = Vec::with_capacity(self.num_of_agents as usize);
        for i in 0..self.num_of_agents {
            let Some(preference_data) = self.preferences.0.get(i as usize) else {
                return Err(SimulationError::NoData);
            };
            agents.push(Agent {
                id: i,
                balance: self.balances.get(i)?,
                preferences: AgentPreferences(preference_data.clone()),
                holding: AgentHoldings(
                    self.holdings
                        .0
                        .iter()
                        .filter(|(key, _)| get_first(**key) == i)
                        .map(|(key, value)| (get_second(*key), *value))
                        .collect(),
                ),
            });
        }
        Ok(agents)
    }
    pub fn set_random_preferences_for_all_companies(
        &mut self,
        rng: &mut impl Rng,
        agent_id: u64,
        num_of_companies: u64,
    ) -> Result<(), SimulationError> {
        let Some(company_preferences) = self.preferences.0.get_mut(agent_id as usize) else {
            return Err(SimulationError::AgentNotFound(agent_id));
        };
        for company_id in 0..num_of_companies {
            let preference: u64 = rng.gen_range(0..100);
            company_preferences.add(&vec![(company_id, TradeAction::Buy); preference as usize]);
        }
        Ok(())
    }
    pub fn give_random_preferences(
        &mut self,
        rng: &mut impl Rng,
        num_of_companies: u64,
    ) -> Result<(), SimulationError> {
        for i in 0..NUM_OF_AGENTS {
            self.set_random_preferences_for_all_companies(rng, i, num_of_companies)?;
        }
        Ok(())
    }
    pub fn introduce_new_rand_agents(
        &mut self,
        rng: &mut impl Rng,
        num_of_agents_to_introduce: u64,
        num_of_companies: u64,
    ) -> Result<(), SimulationError> {
        let mut introduce_ids: Vec<f64> = (self.num_of_agents
            ..(self.num_of_agents + num_of_agents_to_introduce))
            .map(|_| rng.gen_range(1000.0..1_000_000.0))
            .collect();
        self.balances.0.append(&mut introduce_ids);
        self.preferences.0.append(&mut vec![
            Timeline::new();
            num_of_agents_to_introduce as usize
        ]);
        for i in self.num_of_agents..(self.num_of_agents + num_of_agents_to_introduce) {
            self.set_random_preferences_for_all_companies(rng, i, num_of_companies)?;
        }
        self.num_of_agents += num_of_agents_to_introduce;
        Ok(())
    }
    pub fn can_buy(
        &self,
        agent_id: u64,
        price: f64,
        quantity: u64,
    ) -> Result<bool, SimulationError> {
        Ok(self.balances.get(agent_id)? >= price * quantity as f64)
    }
    pub fn can_sell(&self, id: u128, quantity: u64) -> bool {
        self.holdings.get_u128(id) >= quantity
    }
    pub fn iter(&self) -> std::ops::Range<u64> {
        0..self.num_of_agents
    }
    pub fn try_failed_offers(
        &self,
        rng: &mut impl Rng,
        transactions: &mut Vec<TodoTransactions>,
        attempting_trade: &Trade,
    ) -> Result<(), SimulationError> {
        if self.try_offers.is_empty() {
            return Ok(());
        }
        for (id, new_price) in self.try_offers.iter() {
            // 40% chance of retrying
            if rng.gen_ratio(6, 10) {
                continue;
            }
            let (action, price) = if *new_price > 0.0 {
                (TradeAction::Buy, *new_price)
            } else {
                (TradeAction::Sell, -*new_price)
            };
            let can_transact = match action {
                TradeAction::Buy => {
                    self.can_buy(get_first(*id), price, attempting_trade.number_of_shares)?
                }
                TradeAction::Sell => self.can_sell(*id, attempting_trade.number_of_shares),
            };
            if !can_transact {
                continue;
            }
            transactions.push(TodoTransactions {
                agent_id: get_first(*id),
                company_id: get_second(*id),
                strike_price: price,
                action,
                trade: attempting_trade.clone(),
            });
        }
        Ok(())
    }
    pub fn alert_agents(
        &mut self,
        expired_trades: &HashMap<u64, Vec<FailedOffer<Trade>>>,
        expired_options: &HashMap<u64, Vec<FailedOffer<StockOption>>>,
    ) -> Result<(), SimulationError> {
        for (company_id, offers) in expired_trades.iter() {
            for offer in offers.iter() {
                // refund
                if offer.1 == TradeAction::Sell {
                    self.holdings.push(
                        offer.0.lifetime,
                        *company_id,
                        offer.0.data.number_of_shares,
                    );
                } else {
                    self.balances.add(
                        offer.0.offerer_id,
                        offer.0.strike_price * (offer.0.data.number_of_shares as f64),
                    )?;
                }

                self.add_failed_offer(
                    *company_id,
                    offer.0.offerer_id,
                    offer.0.strike_price,
                    &offer.1,
                );
            }
        }
        for (company_id, offers) in expired_options.iter() {
            for offer in offers {
                // refund
                if offer.1 == TradeAction::Sell {
                    self.holdings.push(
                        offer.0.lifetime,
                        *company_id,
                        offer.0.data.number_of_shares,
                    );
                } else {
                    self.balances.add(
                        offer.0.offerer_id,
                        offer.0.strike_price * (offer.0.data.number_of_shares as f64),
                    )?;
                }

                self.add_failed_offer(
                    *company_id,
                    offer.0.offerer_id,
                    offer.0.strike_price,
                    &offer.1,
                );
            }
        }
        Ok(())
    }
    pub fn add_failed_offer(
        &mut self,
        company_id: u64,
        agent_id: u64,
        failed_price: f64,
        offer_type: &TradeAction,
    ) {
        let price = match offer_type {
            TradeAction::Buy => failed_price,
            TradeAction::Sell => -failed_price,
        };
        self.try_offers
            .insert(combine(agent_id, company_id), price + failed_price * 0.25);
    }
    pub fn give_random_assets(
        &mut self,
        rng: &mut impl Rng,
        companies: &Companies,
    ) -> Result<(), SimulationError> {
        for i in 0..NUM_OF_AGENTS {
            self.balances.add(i, rng.gen_range(0.0..1000.0))?;
            let random_company = companies.rand_company_id(rng);
            self.holdings
                .push(i, random_company, random::<u64>() % 1000);
        }
        Ok(())
    }
    pub fn do_transactions(
        &mut self,
        market: &mut Market,
        rng: &mut impl Rng,
        transactions: &mut [TodoTransactions],
    ) -> Result<(), SimulationError> {
        for todo_transaction in transactions.iter() {
            self.trade(
                market,
                rng,
                (todo_transaction.company_id, todo_transaction.agent_id),
                (todo_transaction.strike_price, 5.0),
                &todo_transaction.trade,
                todo_transaction.action,
            )?;
        }
        Ok(())
    }
    pub fn roll_action(
        &self,
        rng: &mut impl Rng,
        agent_id: u64,
        company_id: u64,
        strike_price: f64,
        trade: &Trade,
    ) -> Option<TradeAction> {
        if rng.gen_ratio(1, 2) {
            if !self
                .can_buy(agent_id, strike_price, trade.number_of_shares)
                .unwrap_or(false)
            {
                return None;
            }
            return Some(TradeAction::Buy);
        }
        if !self.can_sell(combine(agent_id, company_id), trade.number_of_shares) {
            return None;
        }
        Some(TradeAction::Sell)
    }
    pub fn trade(
        &mut self,
        market: &mut Market,
        rng: &mut impl Rng,
        (company_id, agent_id): (u64, u64),
        (strike_price, acceptable_strike_price_deviation): (f64, f64),
        trade: &Trade,
        action: TradeAction,
    ) -> Result<(), SimulationError> {
        let result = market.trade(
            agent_id,
            company_id,
            strike_price,
            acceptable_strike_price_deviation,
            trade,
            action,
        );
        if action == TradeAction::Sell {
            self.holdings
                .pop(agent_id, company_id, trade.number_of_shares)?;
        } else {
            if self.balances.get(agent_id)? < strike_price * (trade.number_of_shares as f64) {
                return Err(SimulationError::Unspendable);
            }
            self.balances
                .add(agent_id, -(strike_price * (trade.number_of_shares as f64)))?;
        }

        match result {
            Ok(action_state) => self.handle_action_state(action_state, market, company_id)?,
            Err(offer_idxs) => handle_offer_idxs(
                offer_idxs,
                market,
                rng,
                company_id,
                agent_id,
                strike_price,
                trade,
                action,
            ),
        }
        Ok(())
    }
    pub fn exchange_currency_from_transaction(
        &mut self,
        transaction: &Transaction,
    ) -> Result<(), SimulationError> {
        // seller's holdings and buyer's money are resolved at the time of offering
        self.holdings
            .pop_from_txn(transaction.buyer_id, transaction)?;
        self.balances.add(
            transaction.seller_id,
            transaction.strike_price * (transaction.number_of_shares as f64),
        )?;
        Ok(())
    }
    pub fn handle_action_state(
        &mut self,
        action_state: ActionState,
        market: &mut Market,
        company_id: u64,
    ) -> Result<(), SimulationError> {
        match action_state {
            ActionState::AddedToOffers => {}
            ActionState::InstantlyResolved(transaction) => {
                market.add_transaction(company_id, transaction.strike_price);
                self.exchange_currency_from_transaction(&transaction)?;
            }
            ActionState::PartiallyResolved(transaction) => {
                market.add_transaction(company_id, transaction.strike_price);
                self.exchange_currency_from_transaction(&transaction)?;
            }
        }
        Ok(())
    }
}

fn handle_offer_idxs(
    offer_idxs: Vec<usize>,
    market: &mut Market,
    rng: &mut impl Rng,
    company_id: u64,
    agent_id: u64,
    strike_price: f64,
    trade: &Trade,
    action: TradeAction,
) {
    // 30% chance of accept this offer
    if rng.gen_ratio(7, 10) {
        return;
    }

    let target_offers = match action {
        TradeAction::Buy => &market.house.get_mut_trade_offers(company_id).seller_offers,
        TradeAction::Sell => &market.house.get_mut_trade_offers(company_id).buyer_offers,
    };

    // choose a random offer
    let offer_idx = offer_idxs[random::<usize>() % offer_idxs.len()];
    let offer = target_offers[offer_idx].clone();

    let (transaction, extra_shares_left) =
        market.trade_offer(company_id, &offer, agent_id, trade, action);
    if extra_shares_left > 0 {
        market.house.add_trade_offer(
            agent_id,
            company_id,
            strike_price,
            Trade::new(extra_shares_left),
            action,
        );
    }
    market.add_transaction(company_id, transaction.strike_price);
}
