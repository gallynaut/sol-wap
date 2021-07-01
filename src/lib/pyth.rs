use pyth_client::{
    AccountType, Mapping, Price, PriceStatus, PriceType, Product, MAGIC, PROD_HDR_SIZE, VERSION_2,
};
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[repr(C)]
pub struct UpdatePriceInstruction {
    pub version: u32,
    pub cmd: i32,
    pub status: PriceStatus,
    pub unused: u32,
    pub price: i64,
    pub conf: u64,
    pub pub_slot: u64,
}
impl UpdatePriceInstruction {
    pub fn to_price_result(&self, t: i64) -> PriceResult {
        PriceResult {
            price: self.price,
            conf: self.conf,
            pub_slot: self.pub_slot,
            block_time: t,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PriceResult {
    pub price: i64,
    pub conf: u64,
    pub pub_slot: u64,
    pub block_time: i64,
}

#[derive(Default)]
pub struct ProductResult {
    pub name: String,
    pub key: Pubkey,
    pub price_accounts: [u8; 32],
}
impl fmt::Display for ProductResult {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub struct PriceAccountResult {
    pub key: Pubkey,
    pub expo: i32,
    pub twap: i64,
}

pub trait PythAccount {
    fn is_valid(&self) -> bool;
    // cast byte string into structs
    fn new<T>(d: &[u8]) -> Option<&T> {
        let (_, pxa, _) = unsafe { d.align_to::<T>() };
        if pxa.len() > 0 {
            return Some(&pxa[0]);
        } else {
            return None;
        }
    }
}
impl PythAccount for Mapping {
    fn is_valid(&self) -> bool {
        if self.magic != MAGIC || self.atype != AccountType::Mapping as u32 || self.ver != VERSION_2
        {
            return false;
        }
        true
    }
}
impl PythAccount for Product {
    fn is_valid(&self) -> bool {
        if self.magic != MAGIC || self.atype != AccountType::Product as u32 || self.ver != VERSION_2
        {
            return false;
        }
        true
    }
}
pub trait PythProduct {
    fn get_symbol(&self) -> Option<String>;
    fn decode_attributes(&self) -> Option<HashMap<String, String>>;
}

impl PythProduct for Product {
    fn get_symbol(&self) -> Option<String> {
        let attr_map = match self.decode_attributes() {
            None => return None,
            Some(i) => i,
        };
        let k = String::from("symbol");
        match attr_map.get(&k) {
            Some(i) => return Some(i.clone()),
            None => return None,
        };
    }
    fn decode_attributes(&self) -> Option<HashMap<String, String>> {
        let mut attributes = HashMap::new();
        let mut pr_attr_sz = self.size as usize - PROD_HDR_SIZE;
        let mut pr_attr_it = (&self.attr[..]).iter();
        while pr_attr_sz > 0 {
            let key = get_attr_str(&mut pr_attr_it);
            let val = get_attr_str(&mut pr_attr_it);
            pr_attr_sz -= 2 + key.len() + val.len();
            // println!("{:.<16} {}", key, val);
            attributes.insert(key, val);
        }
        Some(attributes)
    }
}

impl PythAccount for Price {
    fn is_valid(&self) -> bool {
        if self.magic != MAGIC || self.atype != AccountType::Price as u32 || self.ver != VERSION_2 {
            return false;
        }
        let _ = match &self.ptype {
            PriceType::Price => "price",
            _ => return false,
        };
        true
    }
}
impl PythAccount for UpdatePriceInstruction {
    fn is_valid(&self) -> bool {
        let _ = match &self.status {
            PriceStatus::Trading => "trading",
            _ => return false,
        };
        if self.price == 0 {
            return false;
        }
        true
    }
}

pub fn get_attr_str<'a, T>(ite: &mut T) -> String
where
    T: Iterator<Item = &'a u8>,
{
    let mut len = *ite.next().unwrap() as usize;
    let mut val = String::with_capacity(len);
    while len > 0 {
        val.push(*ite.next().unwrap() as char);
        len -= 1;
    }
    return val;
}

pub fn print_products(products: &Vec<ProductResult>) {
    for p in products.iter() {
        println!("{:10} - {}", p.name, p.key)
    }
}
pub fn find_product(products: &Vec<ProductResult>, s: String) -> Option<[u8; 32]> {
    for p in products.iter() {
        if p.name == s {
            return Some(p.price_accounts);
        }
    }
    println!(
        "See {} for a list of symbols",
        "https://pyth.network/markets/"
    );
    None
}
