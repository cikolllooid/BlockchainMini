use std::{thread, time::{Duration, SystemTime}};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
struct Block {
    header: Blockheader,
    index: u32,
    hash: String,
    transaction: Vec<Transaction>,
}

#[derive(Debug, Serialize)]
struct Blockheader {
    time: SystemTime,
    nonce: u32,
    difficulty: u32,
    prev_hash: String,
}

#[derive(Debug, Clone, Serialize)]
struct Transaction {
    sender: String,
    recipient: String,
    amount: f32,
}

struct Chain {
    chain: Vec<Block>,
    curr_trans: Vec<Transaction>,
    difficulty: u32,
    miner_addr: String,
    reward: f32,
}

impl Chain {
    pub fn new(difficulty: u32, miner_addr: String) -> Chain {
        let mut chain = Chain {
            chain: Vec::new(),
            curr_trans: Vec::new(),
            difficulty,
            miner_addr,
            reward: 50.0,
        };
        chain.generate_block();
        chain
    }

    pub fn new_transaction(&mut self, sender: String, recipient: String, amount: f32) -> bool {
        self.curr_trans.push(Transaction {
            sender,
            recipient,
            amount,
        });
        true
    }

    pub fn last_hash(&self) -> String {
        let block = match self.chain.last() {
            Some(block) => block,
            None => return String::from_utf8(vec![48; 64]).unwrap(),
        };
        Chain::hash_block(&block.header)
    }

    pub fn generate_block(&mut self) -> bool {
        let header = Blockheader {
            time: SystemTime::now(),
            nonce: 0,
            difficulty: self.difficulty,
            prev_hash: self.last_hash(),
        };
        let reward_trans = Transaction {
            sender: String::from("root"),
            recipient: self.miner_addr.clone(),
            amount: self.reward,
        };
        let mut block = Block {
            header,
            index: 0,
            hash: String::new(), // Initialize with an empty string
            transaction: vec![],
        };
        block.transaction.push(reward_trans);
        block.transaction.append(&mut self.curr_trans);
        block.index = block.transaction.len() as u32;
        block.hash = Chain::hash_block(&block.header);
        println!("Generated block: {:?}", block);

        Chain::proof_of_work(&mut block);
        self.chain.push(block);
        true
    }

    pub fn proof_of_work(block: &mut Block) {
        let difficulty = block.header.difficulty as u64;
        let delta = 8 / difficulty;
        let progress_thread = thread::spawn(move || {
            for _ in 0..=(1024 / delta) {
                // Here you can update your progress indicator
                thread::sleep(Duration::from_millis(10)); // Fixed interval for waiting
            }
        });

        loop {
            let hash = Chain::hash_block(&block.header);
            let slice = &hash[..block.header.difficulty as usize];
            match slice.parse::<u32>() {
                Ok(val) => {
                    if val != 0 {
                        block.header.nonce += 1;
                    } else {
                        block.hash = hash;
                        break;
                    }
                }
                Err(_) => {
                    block.header.nonce += 1;
                    continue;
                }
            };
        }

        progress_thread.join().unwrap(); // Wait for the progress thread to finish
        println!("Generated block1: {:?}", block);
        println!("Block mined: {}", block.hash);
    }

    fn hash_block(header: &Blockheader) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", header).as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn main() {
    let mut chain = Chain::new(1, "loshara".to_string());
    let sender = "Alice".to_string();
    let recipient = "Bob".to_string();
    let amount = 10.0;
    chain.new_transaction(sender, recipient, amount);
    chain.generate_block();

}
