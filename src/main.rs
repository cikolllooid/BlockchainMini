use rand::{distributions::{Distribution, WeightedIndex}, prelude::*};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use ring::{
    rand as ring_rand,
    signature::{self, KeyPair},
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    sysvar::{clock::Clock, Sysvar},
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize, Clone)]
struct Block {
    header: PohRecord,
    index: u32,
    transaction: Vec<Transaction>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Serialize, Deserialize)]
struct PohRecord {
    time: u128,
    prev_hash: String,
    hash: String,
    selected_user: usize,
    validator: Vec<u8>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    sender: String,
    recipient: String,
    amount: f32,
    signature: Vec<u8>,
}


#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug)]
struct Chain {
    chain: Vec<Block>,
    curr_trans: Vec<Transaction>,
    users: HashMap<String, User>,
}


#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug)]
struct User {
    id: usize,
    email: String,
    username: String,
    password: String,
    balance: f32,
    public_key: Vec<u8>,
}


impl User {
    pub fn new(
        id: usize,
        email: String,
        username: String,
        password: String,
        balance: f32,
        public_key: Vec<u8>,
    ) -> Self {
        User {
            id,
            email,
            username,
            password,
            balance,
            public_key,
        }
    }

    pub fn update_balance(&mut self, amount: f32) {
        self.balance += amount;
    }
}
static USER_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

impl Chain {
    pub fn new() -> Chain {
        Chain {
            chain: Vec::new(),
            curr_trans: Vec::new(),
            users: HashMap::new(),
        }
    }

    pub fn create_user(
        &mut self,
        email: String,
        username: String,
        password: String,
        balance: f32,
        public_key_bytes: Vec<u8>,
    ) {
        let id = USER_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        self.users.insert(
            email.clone(),
            User::new(id, email, username, password, balance, public_key_bytes),
        );
    }

    pub fn new_transaction(
        &mut self,
        sender: String,
        recipient: String,
        amount: f32,
        key_pair: &signature::Ed25519KeyPair,
    ) -> bool {
        if let Some(sender_user) = self.users.get(&sender) {
            if sender_user.balance < amount {
                println!("Insufficient balance for sender: {}", sender);
                return false;
            }
        } else {
            println!("Sender not found: {}", sender);
            return false;
        }
    
        if !self.users.contains_key(&recipient) {
            println!("Recipient not found: {}", recipient);
            return false;
        }
    
        let transaction = Transaction {
            sender: sender.clone(),
            recipient: recipient.clone(),
            amount,
            signature: Vec::new(), // Нужно подписать транзакцию перед добавлением в curr_trans
        };
    
        let message = serde_json::to_string(&transaction).unwrap();
        let signature = key_pair.sign(message.as_bytes());
        let transaction = Transaction {
            sender,
            recipient,
            amount,
            signature: signature.as_ref().to_vec(),
        };
        self.curr_trans.push(transaction);
        self.generate_block(); // Генерируем блок только если транзакция успешно добавлена в curr_trans
        true
    }
    
    pub fn last_hash(&self) -> String {
        self.chain.last().map_or_else(
            || String::from_utf8(vec![48; 64]).unwrap(),
            |block| block.header.hash.clone(),
        )
    }

    pub fn generate_block(&mut self) -> bool {
        if self.curr_trans.is_empty() {
            return false;
        }

        let reward_trans = Transaction {
            sender: "system".to_string(),
            recipient: "miner".to_string(),
            amount: 1.0,
            signature: Vec::new(),
        };

        let mut block = Block {
            header: PohRecord {
                time: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis(),
                prev_hash: self.last_hash(),
                hash: String::new(),
                selected_user: 0,
                validator: Vec::new(),
            },
            index: (self.chain.len() as u32) + 1,
            transaction: vec![reward_trans],
        };

        let mut valid_transactions = vec![];
        for transaction in &self.curr_trans {
            if self.verify_transaction(transaction) {
                valid_transactions.push(transaction.clone());
            } else {
                println!("Invalid transaction: {:?}", transaction);
            }
        }
        let (public_key, user_id) = Chain::user_wb(&self);

        block.transaction.append(&mut valid_transactions);
        block.header.hash = Chain::proof_of_stake(&block);
        block.header.validator = public_key;
        block.header.selected_user = user_id;

        println!("Generated block: {:?}", block);
        self.update_balances(&block);
        self.chain.push(block);
        self.curr_trans.clear();
        true
    }

    fn user_wb(&self) -> (Vec<u8>, usize){
        let total_stake: f32 = self.users.values().map(|a| a.balance).sum();
        let selection_probabilities: Vec<f32> = self.users.values().map(|a: &User| a.balance / total_stake).collect();
        let dist = WeightedIndex::new(&selection_probabilities).unwrap();
        let mut rng = thread_rng();
        
        let selected_index = dist.sample(&mut rng);
        let keys: Vec<&String> = self.users.keys().collect();
        let selected_key = keys.get(selected_index).unwrap();

        let selected_user = self.users.get(*selected_key).unwrap();
        let public_key_clone = selected_user.public_key.clone();
        let user_id_clone = selected_user.id.clone();

        (public_key_clone, user_id_clone)
    }

    fn proof_of_stake(block: &Block) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", block).as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        hash
    }
    

    pub fn verify_transaction(&self, transaction: &Transaction) -> bool {
        if let Some(sender_user) = self.users.get(&transaction.sender) {
            let message = serde_json::to_string(&Transaction {
                sender: transaction.sender.clone(),
                recipient: transaction.recipient.clone(),
                amount: transaction.amount,
                signature: vec![],
            })
            .unwrap();
            let public_key = signature::UnparsedPublicKey::new(&signature::ED25519, sender_user.public_key.clone());
            public_key.verify(message.as_bytes(), &transaction.signature).is_ok()
        } else {
            false
        }
    }
    

    pub fn update_balances(&mut self, block: &Block) {
        for transaction in &block.transaction {
            if let Some(sender_user) = self.users.get_mut(&transaction.sender) {
                sender_user.update_balance(-transaction.amount);
            }
            if let Some(recipient_user) = self.users.get_mut(&transaction.recipient) {
                recipient_user.update_balance(transaction.amount);
            }
        }
    }
    pub fn find_transaction(&self, index: u32) -> Option<&Block> {
        if let Some(block) = self.chain.iter().find(|block| block.index == index) {
            println!("Found block: {:?}", block);
            Some(block)
        } else {
            println!("Block not found!");
            None
        }
    }

    pub fn is_chain_valid(&self) -> bool {
        if self.chain.is_empty() {
            return true;
        }
    
        let chain_clone = self.chain.clone();
        let previous_block = &chain_clone[0];
    
        for current_block in chain_clone.iter().skip(1) {
            let current_block_hash = &current_block.header.prev_hash;
            let expected_previous_hash = &previous_block.header.hash;
    
            if current_block_hash != expected_previous_hash {
                println!("the previous hash of the block is not equal to the present one");
                return false;
            }
        }
    
        true
    }

    pub fn find_user_by_id(&self, id: usize) -> Option<&User>{
        let finder_user = self.users.values();
        let goil = finder_user.into_iter().find(|&user| user.id == id).clone();
        goil
    }
    
}

fn main() {
    let mut chain = Chain::new();
    let rng = ring_rand::SystemRandom::new();
    let key_pair = signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(key_pair.as_ref()).unwrap();
    let public_key_bytes = key_pair.public_key().as_ref().to_vec();
    chain.create_user(
        "sigma@b.ru".to_string(),
        "Alice".to_string(),
        "12345".to_string(),
        100.0,
        public_key_bytes.clone(),
    );
    chain.create_user(
        "sigma1@b.ru".to_string(),
        "Bob".to_string(),
        "12345".to_string(),
        50.0,
        public_key_bytes.clone(),
    );
    let sender = "sigma@b.ru".to_string();
    let recipient = "sigma1@b.ru".to_string();
    let amount = 10.0;

    chain.new_transaction(sender.clone(), recipient.clone(), amount, &key_pair);

    let serialized_data = chain.try_to_vec().unwrap();

    let accounts: Vec<AccountInfo> = vec![];
    let program_id = Pubkey::default();

    let result = process_instruction(&program_id, &accounts, &serialized_data);

    match result {
        Ok(_) => println!("Instruction processed successfully"),
        Err(err) => eprintln!("Error processing instruction: {:?}", err),
    }
}
