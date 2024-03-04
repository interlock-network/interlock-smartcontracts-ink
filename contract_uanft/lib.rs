//!
//! # INTERLOCK NETWORK - UNIVERSAL ACCESS NFT
//!
//! This is a PSP34 NFT in compatible with Art Zero marketplace and capable of managing user access
//! credentials on the blockchain using a strategy similar to two-factor-authentication (2FA).
//!
//! Build needs cargo-contract version 2.0.0:
//!
//! -     cargo install cargo-contract --force --version 2.0.0
//!
//! To build:
//!
//! -     cargo +nightly-2023-02-07 contract build
//!
//!  To build docs:
//!
//! -     cargo +nightly doc --no-deps --document-private-items --open
//!
//! To reroute docs in Github:
//!
//! -     echo "<meta http-equiv=\"refresh\" content=\"0; url=build_wheel\">" >
//! -     target/doc/index.html;
//! -     cp -r target/doc ./docs
//!

#![doc(
    html_logo_url = "https://assets-global.website-files.com/64d9930f57641d176ab09b78/64dde3b1459a01ddf7b4a529_interlock-logo-large.webp",
    html_favicon_url = "https://assets-global.website-files.com/64d9930f57641d176ab09b78/64da50c8875e833f16060147_Favicon.png",
)]

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

pub use self::uanft::{Psp34Nft, Psp34NftRef};

#[openbrush::contract]
pub mod uanft {

    // ink 4 imports
    use ink::{
        codegen::{Env, EmitEvent},
        storage::Mapping,
        prelude::{
            string::{String, ToString},
            vec::Vec,
            format,
        },
        reflect::ContractEventBase,
    };

    // openbrush 3 imports
    use openbrush::{
        traits::Storage,
        modifiers,
        contracts::{
            ownable::*,
            psp34::{
                extensions::{
                    enumerable::*,
                    metadata::*,
                    burnable::*,
                },
                Internal,
                PSP34Error,
            },
            pausable::*,
        },
    };

    // we use these to interface as uanft application
    // with the Interlock Network PSP22 contract
    use ilockmvp::{
        ilockmvp::OtherError,
        ILOCKmvpRef,
    };

    /// - Constants.
    pub const TIME_LIMIT_MIN: Timestamp = 600_000;      // ten minutes
    pub const THRESHOLD_MIN: u16 = 2;                   // two signers 
    

    /// - Multisig functions.
    pub const TRANSFER_OWNERSHIP: u8    = 0;
    pub const UNPAUSE: u8               = 1;
    pub const ADD_SIGNATORY: u8         = 2;
    pub const REMOVE_SIGNATORY: u8      = 3;
    pub const CHANGE_TIMELIMIT: u8      = 4;
    pub const CHANGE_THRESHOLD: u8      = 5;
    pub const UPDATE_CONTRACT: u8       = 6;

    #[openbrush::wrapper]
    pub type Psp34Ref = dyn PSP34 + PSP34Metadata;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InvalidInput,
        Custom(String),
        OwnableError(OwnableError),
        PSP34Error(PSP34Error),
    }
    impl From<OwnableError> for Error {
        fn from(ownable: OwnableError) -> Self {
            Error::OwnableError(ownable)
        }
    }
    impl From<PSP34Error> for Error {
        fn from(error: PSP34Error) -> Self {
            Error::PSP34Error(error)
        }
    }


    /// This is a type wrapper to implement Default method
    /// on AccountId type. Ink 4 stable eliminated AccountId Default
    /// (which was zero address, that has known private key)
    /// ...we only really need this because Openbrush contract
    ///    relies on deriving Default for contract storage, and
    ///    our AccesData struct contains AccountId.
    #[derive(scale::Encode, scale::Decode, Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(
        feature = "std",
        derive(
            Eq,
            scale_info::TypeInfo,
            ink::storage::traits::StorageLayout,
        )
    )]
    pub struct AccountID {
        address: AccountId,
    }
    impl Default for AccountID {
        fn default() -> AccountID {
            AccountID {
                address: AccountId::from([1_u8;32]),
            }
        }
    }

    /// - This is upgradable storage for the multisig feature of this
    /// PSP22 contract (ie, the application/socket/port contract connectivity formalism).
    pub const MULTISIG_KEY: u32 = openbrush::storage_unique_key!(MultisigData);
    #[derive(Default, Debug)]
    #[openbrush::upgradeable_storage(MULTISIG_KEY)]
    pub struct MultisigData {

        // ABSOLUTELY DO NOT CHANGE THE ORDER OF THESE VARIABLES
        // OR TYPES IF UPGRADING THIS CONTRACT!!!

        /// - Stanging transaction
        pub tx: Transaction,

        /// - Vector of signatories.
        pub signatories: Vec<AccountID>,
        
        /// - Multisig threshold..
        pub threshold: u16,

        /// - Multisig time limit.
        pub timelimit: Timestamp,

        /// - Expand storage related to the multisig functionality.
        pub _reserved: Option<()>,
    }
    /// - TransactionData struct contains all pertinent information for multisigtx transaction
    #[derive(scale::Encode, scale::Decode, Clone, Default, Debug)]
    #[cfg_attr(
    feature = "std",
    derive(
        PartialEq,
        Eq,
        scale_info::TypeInfo,
        ink::storage::traits::StorageLayout,
        )
    )]

    pub struct Transaction {

        // ABSOLUTELY DO NOT CHANGE THE ORDER OF THESE VARIABLES
        // OR TYPES IF UPGRADING THIS CONTRACT!!!

        /// - Which signatory ordered the multisigtx tx?
        pub orderer: AccountID,

        /// - What signatures have been collected?
        pub signatures: Vec<Signature>,

        /// - Which multisigtx function is being called?
        pub function: u8,

        /// - What is the timestamp on current transaction?
        pub time: Timestamp,

        /// - Was transaction completed?
        pub ready: bool,
    }
    /// - TransactionData struct contains all pertinent information for multisigtx transaction
    #[derive(scale::Encode, scale::Decode, Clone, Copy, Default, Debug)]
    #[cfg_attr(
    feature = "std",
    derive(
        PartialEq,
        Eq,
        scale_info::TypeInfo,
        ink::storage::traits::StorageLayout,
        )
    )]
    pub struct Signature {

        // ABSOLUTELY DO NOT CHANGE THE ORDER OF THESE VARIABLES
        // OR TYPES IF UPGRADING THIS CONTRACT!!!

        /// - Who signed this signature?
        pub signer: AccountID,

        /// - What is the timestamp on current transaction?
        pub time: Timestamp,
    }

    /// - This is upgradable storage for the access features for this
    /// universal access nft contract.
    pub const ACCESS_KEY: u32 = openbrush::storage_unique_key!(AccessData);
    #[derive(Default, Debug)]
    #[openbrush::upgradeable_storage(ACCESS_KEY)]
    pub struct AccessData {

        /// uanft token cap
        pub cap: u64,

        /// nft sale price in ILOCK (or other) PSP22 token
        pub nft_psp22price: Balance,

        /// - Collections contains information about which uanft IDs a particular
        /// address holds.
        /// - This in part is important because it provides information
        /// about how many different access credential instances a particular wallet
        /// owner has for a given access usecase.
        ///
        /// collections:         user accress -> vector of uanft IDs in collection
        pub collections: Mapping<AccountId, Vec<Id>>,

        /// - This is to expand storage related to this uanft's access functionality.
        pub _reserved: Option<()>
    }

    /// - This is upgradable storage for the features that allow this universal
    /// access nft contract to connect as an application to the ILOCK (or other)
    /// PSP22 contract the application socket abstraction.
    pub const APP_KEY: u32 = openbrush::storage_unique_key!(AppData);
    #[derive(Default, Debug)]
    #[openbrush::upgradeable_storage(APP_KEY)]
    pub struct AppData {

        /// - This is PSP22 token contract that this uanft application connects to via socket.
        /// - This is used for self-minting purposes, which means non-owner can
        /// mint in excange for PSP22 token (ILOCK in this case) without needing
        /// to rely on a transaction relay server off-chain.
        pub token_instance: ILOCKmvpRef,

        /// - This is address that manages this uanft contract and receives ILOCK
        /// (or other) PSP22 token for self-mint transactions.
        pub operator: AccountID,

        /// - This is to expand storage related to this uanft application functionality.
        pub _reserved: Option<()>
    }
    /// - This is the port number for this type of uanft application socket connections to ILOCK (or other)
    /// PSP22 token contract.
    /// - PORT 0 designates uanft contracts owned by Interlock Network.
    /// - This port is locked by default (only Interlock Network may connect this uanft
    /// contract via socket to ILOCK PSP22 contrac.
    pub const PORT: u16 = 0;

    /// - Main contract storage.
    #[derive(Default, Storage)]
    #[ink(storage)]
    pub struct Psp34Nft {

        /// - Openbrush PSP34 storage fields.
        #[storage_field]
        psp34: psp34::Data<enumerable::Balances>,

        /// - Openbrush metadata extension storage fields.
        #[storage_field]
        metadata: metadata::Data,

        /// - Openbrush ownable extension storage fields.
        #[storage_field]
        ownable: ownable::Data,

        /// - Openbrush pausable extensios.
        #[storage_field]
		pausable: pausable::Data,

        /// - Universal access NFT storage fields.
        #[storage_field]
        access: AccessData,

        /// - Storage fields related to the UANFT as an application for the ILOCK PSP22 contract.
        #[storage_field]
        app: AppData,

        /// - ILOCK multisig contract info.
        #[storage_field]
        multisig: MultisigData,

        /// - Art zero storage fields.
        last_token_id: u64,
        attribute_count: u32,
        attribute_names: Mapping<u32, Vec<u8>>,
        locked_tokens: Mapping<Id, bool>,
        locked_token_count: u64,
        is_attribute: Mapping<String, bool>,

    }

    /// - Specify transfer event.
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        pub from: Option<AccountId>,
        #[ink(topic)]
        pub to: Option<AccountId>,
        pub id: Id,
    }

    /// - Specify approval event.
    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        pub from: Option<AccountId>,
        #[ink(topic)]
        pub to: Option<AccountId>,
        pub id: Id,
        pub approved: bool,
    }

    /// - For Pausable functions that are only_owner.
    impl From<PausableError> for Error {
        fn from(error: PausableError) -> Self {
            Error::Custom(format!("{:?}", error))
        }
    }

    /// - For cross contract other error functions.
    impl From<OtherError> for Error {
        fn from(error: OtherError) -> Self {
            Error::Custom(format!("{:?}", error))
        }
    }

    /// - Needed for Openbrush internal event emission implementations.
    pub type Event = <Psp34Nft as ContractEventBase>::Type;

    impl Internal for Psp34Nft {

        /// - Impliment Transfer emit event because Openbrush doesn't.
        fn _emit_transfer_event(
            &self,
            _from: Option<AccountId>,
            _to: Option<AccountId>,
            _id: Id,
        ) {
            Psp34Nft::emit_event(
                self.env(),
                Event::Transfer(Transfer {
                    from: _from,
                    to: _to,
                    id: _id,
                }),
            );
        }

        /// - Impliment Approval emit event because Openbrush doesn't.
        fn _emit_approval_event(
            &self,
            _from: AccountId,
            _to: AccountId,
            _id: Option<Id>,
            _approved: bool,
        ) {
            Psp34Nft::emit_event(
                self.env(),
                Event::Approval(Approval {
                    from: Some(_from),
                    to: Some(_to),
                    id: _id.unwrap(),
                    approved: _approved,
                }),
            );
        }
    }

    impl PSP34 for Psp34Nft {

        /// - Override transfer function to revoke access credentials if existent.
        /// - This also updates collection.
        #[ink(message)]
        #[openbrush::modifiers(when_not_paused)]
        fn transfer(
            &mut self,
            to: AccountId,
            id: Id,
            data: Vec<u8>
        ) -> Result<(), PSP34Error> {

            // transfer
            let from = self.env().caller();
            let _ = self._transfer_token(to, id.clone(), data)?;

            // update sender's collection
            let mut from_collection = match self.access.collections.get(from) {
                Some(collection) => collection,
                None => return Err(PSP34Error::Custom(
                        format!("No collection, fatal error").into_bytes())),
            };
            let index = match from_collection.iter().position(|element| *element == id) {
                Some(index) => index,
                None => return Err(PSP34Error::Custom(
                        format!("token not in collection").into_bytes())),
            };
            from_collection.remove(index);
            self.access.collections.insert(from, &from_collection);

            // update recipient's collection
            let mut to_collection = match self.access.collections.get(to) {
                Some(collection) => collection,
                None => Vec::new(),
            };
            to_collection.push(id);
            self.access.collections.insert(to, &to_collection);

            Ok(())
        }
    }

    impl Ownable for Psp34Nft {
        
        /// - Nobody can transfer ownership from this implementation..calling does nothing.
        /// - Transfer ownership implemented before update_contract() with multisigtx
        #[ink(message)]
        fn transfer_ownership(&mut self, _newowner: AccountId) -> Result<(), OwnableError> {

            // do nothing
            Ok(())
        }

        /// - Nobody can renounce ownership.
        #[ink(message)]
        fn renounce_ownership(&mut self) -> Result<(), OwnableError> {

            Ok(()) 
        }
    }

    impl PSP34Metadata for Psp34Nft {}
    impl PSP34Enumerable for Psp34Nft {}

    impl PSP34Burnable for Psp34Nft {

        /// - Art Zero message.
        ///
        #[ink(message)]
        #[openbrush::modifiers(when_not_paused)]
        fn burn(&mut self, account: AccountId, id: Id) -> Result<(), PSP34Error> {
            let caller = self.env().caller();
            let token_owner = self.owner_of(id.clone()).unwrap();
            if token_owner != account {
                return Err(PSP34Error::Custom(String::from("not token owner").into_bytes()))
            }

            let allowance = self.allowance(account,caller,Some(id.clone()));

            if caller == account || allowance {
                self.locked_tokens.remove(&id);
                self.locked_token_count = self.locked_token_count.checked_sub(1).unwrap();
                self._burn_from(account, id)
            } else{
                Err(PSP34Error::Custom(String::from("caller is not token owner or approved").into_bytes()))
            }
        }
    }

    /// - Art Zero trait definitions.
    ///
    #[openbrush::trait_definition]
    pub trait Psp34Traits {

        #[ink(message)]
        fn get_last_token_id(&self) -> u64;

        #[ink(message)]
        fn lock(&mut self, token_id: Id) -> Result<(), Error>;

        #[ink(message)]
        fn is_locked_nft(&self, token_id: Id) -> bool;

        #[ink(message)]
        fn get_locked_token_count(&self) -> u64;

        #[ink(message)]
        #[modifiers(only_owner)]
        fn set_base_uri(&mut self, uri: String) -> Result<(), Error>;

        #[ink(message)]
        #[modifiers(only_owner)]
        fn set_multiple_attributes(&mut self, token_id: Id, metadata: Vec<(String, String)>) -> Result<(), Error>;

        #[ink(message)]
        fn get_attributes(&self, token_id: Id, attributes: Vec<String>) -> Vec<String>;
        
        #[ink(message)]
        fn get_attribute_count(&self) -> u32;
        
        #[ink(message)]
        fn get_attribute_name(&self, index: u32) -> String;
        
        #[ink(message)]
        fn token_uri(&self, token_id: u64) -> String;
        
        #[ink(message)]
        fn get_owner(&self) -> AccountId;
    }

    /// - Convenience Result Type
    pub type OtherResult<T> = core::result::Result<T, Error>;

    impl Psp34Nft {

        /// - Function for internal _emit_event implementations.
        pub fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event);
        }

        /// - UANFT contract constructor.
        #[ink(constructor)]
        pub fn new(
            name: String,
            symbol: String,
            class: String,
            cap: u64,
            price: Balance,
            token_address: AccountId,
            timelimit: Timestamp,
            signatory_2: AccountId,
            signatory_3: AccountId,
        ) -> Self {
            
            // create the contract
            let mut contract = Self::default();
                                
            // define owner as caller
            let caller = contract.env().caller();

            // use panic for errors in constructor

            // owner cannot be double listed as signatory
            if caller == signatory_2 || caller == signatory_3 {
                panic!("caller is signatory");
            }

            // cannot construct with both signantories the same
            if signatory_2 == signatory_3 {
                panic!("signatories are the same");
            }

            if timelimit < TIME_LIMIT_MIN {
                panic!("timelimit too small");
            }

            // define first three signatory
            let firstsignatory: AccountID = AccountID { address: caller };
            let secondsignatory: AccountID = AccountID { address: signatory_2 };
            let thirdsignatory: AccountID = AccountID { address: signatory_3 };

            // push first two signatories
            contract.multisig.signatories.push(firstsignatory);
            contract.multisig.signatories.push(secondsignatory);
            contract.multisig.signatories.push(thirdsignatory);

            // set attributes
            contract._set_attribute(
                Id::U8(0),
                String::from("name").into_bytes(),
                name.into_bytes(),
            );
            contract._set_attribute(
                Id::U8(0),
                String::from("symbol").into_bytes(),
                symbol.into_bytes(),
            );
            contract._set_attribute(
                Id::U8(0),
                String::from("class").into_bytes(),
                class.into_bytes(),
            );

            // multisig defaults
            contract.multisig.timelimit = timelimit;
            contract.multisig.threshold = 2;

            // assign caller as owner
            contract._init_with_owner(caller);

            // create a reference to the deployed PSP22 ILOCK token contract
            contract.app.token_instance = ink::env::call::FromAccountId::from_account_id(token_address);
            contract.app.operator.address = caller;

            // set cap
            contract.access.cap = cap;

            // set nft price in PSP22 token
            contract.access.nft_psp22price = price;

            contract
        }

////////////////////////////////////////////////////////////////////////////
/////// multisigtx /////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////
//
// Workflow:
//
// 1) signatory orders multisig transaction via order_multisigtx()
//      ...this signatory's order is considered the first signature
// 2) other signatories sign multisig transaction via sign_multisigtx()
// 3) any signatory may check the number of signatures
// 4) when signature count threshold is met, then any signatory may call specified function
//
// - all signatories must agree on the function they are signing for (ie, the multisigtx ordered)
// - to prevent case where corrupted signatory exists, no signatory may order a multisigtx
//   consecutively. This is to prevent corrupted signatory from jamming up the multisig process
//

        /// - Helper function for checking signature count
        pub fn check_multisig(
            &self,
            function: String,
        ) -> OtherResult<()> {

            let caller: AccountID = AccountID { address: self.env().caller() };
            let thistime: Timestamp = self.env().block_timestamp();

            // make sure caller is designated multisigtx account
            if !self.multisig.signatories.contains(&caller) {

                return Err(Error::Custom(format!("CallerNotSignatory")));
            }

            // if enough signatures had not been supplied, revert
            if self.multisig.tx.signatures.len() < self.multisig.threshold as usize {

                return Err(Error::Custom(format!("NotEnoughSignatures")));
            }

            // if multisigtx is too old, then signature does not matter
            if thistime - self.multisig.tx.time >= self.multisig.timelimit {

                return Err(Error::Custom(format!("TransactionStale")));
            }

            // get function index
            let function: u8 = match function.as_str() {
                "TRANSFER_OWNERSHIP"    => TRANSFER_OWNERSHIP,
                "UNPAUSE"               => UNPAUSE,
                "ADD_SIGNATORY"         => ADD_SIGNATORY,
                "REMOVE_SIGNATORY"      => REMOVE_SIGNATORY,
                "CHANGE_THRESHOLD"      => CHANGE_THRESHOLD,
                "CHANGE_TIMELIMIT"      => CHANGE_TIMELIMIT,
                "UPDATE_CONTRACT"       => UPDATE_CONTRACT,
                _ => return Err(Error::Custom(format!("InvalidFunction"))),
            };

            // signer must know they are signing for the right function
            if function != self.multisig.tx.function {

                return Err(Error::Custom(format!("WrongFunction")));
            }

            Ok(())
        }

        /// - Function to order multisigtx transaction.
        #[ink(message)]
        pub fn order_multisigtx(
            &mut self,
            function: String,
        ) -> OtherResult<()> {

            let caller: AccountID = AccountID { address: self.env().caller() };
            let thistime: Timestamp = self.env().block_timestamp();

            // make sure caller is designated multisigtx account
            if !self.multisig.signatories.contains(&caller) {

                return Err(Error::Custom(format!("CallerNotSignatory")));
            }

            // if the signing period has already begun, orderer
            if thistime - self.multisig.tx.time < self.multisig.timelimit {

                return Err(Error::Custom(format!("TransactionAlreadyOrdered")));
            }

            // this is important to prevent corrupted key from 'freezing out'
            // other signatories' ability to order transaction
            if thistime - self.multisig.tx.time >= self.multisig.timelimit
                && caller == self.multisig.tx.orderer {

                return Err(Error::Custom(format!("CannotReorder")));
            }

            // get function index
            let function: u8 = match function.as_str() {
                "TRANSFER_OWNERSHIP"    => TRANSFER_OWNERSHIP,
                "UNPAUSE"               => UNPAUSE,
                "ADD_SIGNATORY"         => ADD_SIGNATORY,
                "REMOVE_SIGNATORY"      => REMOVE_SIGNATORY,
                "CHANGE_THRESHOLD"      => CHANGE_THRESHOLD,
                "CHANGE_TIMELIMIT"      => CHANGE_TIMELIMIT,
                "UPDATE_CONTRACT"       => UPDATE_CONTRACT,
                _ => return Err(Error::Custom(format!("InvalidFunction"))),
            };

            // set transaction function
            self.multisig.tx.function = function;

            // set transaction order time
            self.multisig.tx.time = thistime;

            // construct signature
            let signature: Signature = Signature {
                signer: caller,
                time: thistime,
            };

            // add first signature to multisigtx transaction order
            self.multisig.tx.signatures = Vec::new();
            self.multisig.tx.signatures.push(signature);

            Ok(())
        }

        /// - A multisigtx signer calls this to sign.
        #[ink(message)]
        pub fn sign_multisigtx(
            &mut self,
            function: String,
        ) -> OtherResult<()> {

            let caller: AccountID = AccountID { address: self.env().caller() };
            let thistime: Timestamp = self.env().block_timestamp();

            // make sure caller is designated multisigtx account
            if !self.multisig.signatories.contains(&caller) {

                return Err(Error::Custom(format!("CallerNotSignatory")));
            }

            // get function index
            let function: u8 = match function.as_str() {
                "TRANSFER_OWNERSHIP"    => TRANSFER_OWNERSHIP,
                "UNPAUSE"               => UNPAUSE,
                "ADD_SIGNATORY"         => ADD_SIGNATORY,
                "REMOVE_SIGNATORY"      => REMOVE_SIGNATORY,
                "CHANGE_THRESHOLD"      => CHANGE_THRESHOLD,
                "CHANGE_TIMELIMIT"      => CHANGE_TIMELIMIT,
                "UPDATE_CONTRACT"       => UPDATE_CONTRACT,
                _ => return Err(Error::Custom(format!("InvalidFunction"))),
            };

            // signer must know they are signing for the right function
            if function != self.multisig.tx.function {

                return Err(Error::Custom(format!("WrongFunction")));
            }

            // if multisigtx is too old, then signature does not matter
            if thistime - self.multisig.tx.time >= self.multisig.timelimit {

                return Err(Error::Custom(format!("TransactionStale")));
            }

            // make sure signatory has not already signed for the transaction
            if self.multisig.tx.signatures.iter().any(|sig| sig.signer == caller) {

                return Err(Error::Custom(format!("AlreadySigned")));
            }

            // construct signature
            let signature: Signature = Signature {
                signer: caller,
                time: thistime,
            };

            self.multisig.tx.signatures.push(signature);

            Ok(())
        }

        /// - This adds a signatory from the list of permitted signatories.
        #[ink(message)]
        pub fn add_signatory(
            &mut self,
            signatory: AccountId,
            function: String,
        ) -> OtherResult<()> {
    
            // verify multisig good
            let _ = self.check_multisig(function)?;

            // make sure signatory is not zero address
            if signatory == AccountId::from([0_u8; 32]) {

                return Err(Error::Custom(format!("IsZeroAddress")));
            }

            let signatory: AccountID = AccountID { address: signatory };

            // make sure caller is designated multisigtx account
            if self.multisig.signatories.contains(&signatory) {

                return Err(Error::Custom(format!("AlreadySignatory")));
            }

            self.multisig.signatories.push(signatory);

            Ok(())
        }

        /// - This removes a signatory from the list of permitted signatories.
        #[ink(message)]
        pub fn remove_signatory(
            &mut self,
            signatory: AccountId,
            function: String,
        ) -> OtherResult<()> {
        
            // verify multisig good
            let _ = self.check_multisig(function)?;

            // make sure signatory is not zero address
            if signatory == AccountId::from([0_u8; 32]) {

                return Err(Error::Custom(format!("IsZeroAddress")));
            }

            let signatory: AccountID = AccountID { address: signatory };

            // make sure signatory is designated multisigtx account
            if !self.multisig.signatories.contains(&signatory) {

                return Err(Error::Custom(format!("NoSignatory")));
            }

            // contract must maintain THRESHOLD + 1 signatories at all times
            let neededsignatories: u16 = match self.multisig.threshold.checked_add(1) {
                Some(sum) => sum,
                None => return Err(Error::Custom(format!("Overflow"))),
            };

            // make sure there are enough signatories for new threshold
            if self.multisig.signatories.len() <= neededsignatories.into() {

                return Err(Error::Custom(format!("TooFewSignatories")));
            }

            self.multisig.signatories.retain(|&account| account != signatory);

            Ok(())
        }

        /// - This changes signer threshold for approving multisigtx.
        #[ink(message)]
        pub fn change_threshold(
            &mut self,
            threshold: u16,
            function: String,
        ) -> OtherResult<()> {
    
            // verify multisig good
            let _ = self.check_multisig(function)?;

            // make sure new threshold is greater then minimum
            if threshold < THRESHOLD_MIN {

                return Err(Error::Custom(format!("UnderThresholdMin")));
            }

            // contract must maintain THRESHOLD + 1 signatories at all times
            let neededsignatories: u16 = match threshold.checked_add(1) {
                Some(sum) => sum,
                None => return Err(Error::Custom(format!("Overflow"))),
            };

            // make sure there are enough signatories for new threshold
            if self.multisig.signatories.len() < neededsignatories.into() {

                return Err(Error::Custom(format!("TooFewSignatories")));
            }

            self.multisig.threshold = threshold;

            Ok(())
        }

        /// - This modifies timelimit for a multisig transaction.
        #[ink(message)]
        pub fn change_multisigtxtimelimit(
            &mut self,
            timelimit: Timestamp,
            function: String,
        ) -> OtherResult<()> {
    
            // verify multisig good
            let _ = self.check_multisig(function)?;

            // make sure limit is respected
            if timelimit < TIME_LIMIT_MIN {

                return Err(Error::Custom(format!("UnderTimeLimit")));
            }

            self.multisig.timelimit = timelimit;

            Ok(())
        }

        /// - This gets the current signature threshold for multisigtx.
        #[ink(message)]
        pub fn threshold(
            &self,
        ) -> u16 {

            self.multisig.threshold
        }

        /// - This gets the current timelimit for signatories to sign multisigtx.
        #[ink(message)]
        pub fn multisigtimelimit(
            &self,
        ) -> Timestamp {

            self.multisig.timelimit
        }

        /// - This gets a list of current accounts permitted to sign multisigtx.
        #[ink(message)]
        pub fn signatories(
            &mut self,
        ) -> OtherResult<Vec<AccountID>> {

            let caller: AccountID = AccountID { address: self.env().caller() };

            // make sure caller is designated multisigtx account
            if !self.multisig.signatories.contains(&caller) {

                return Err(Error::Custom(format!("CallerNotSignatory")));
            }
            
            Ok(self.multisig.signatories.iter().map(|sig| *sig ).collect())
        }

        /// - This gets number of signatories permitted to sign multisigtx.
        #[ink(message)]
        pub fn signatory_count(
            &self,
        ) -> u8 {

            self.multisig.signatories.len() as u8
        }

        /// - This gets current number of signatures for multisigtx.
        #[ink(message)]
        pub fn signature_count(
            &self,
        ) -> u8 {

            self.multisig.tx.signatures.len() as u8
        }

        /// - This gets a list of all signers so far on a multisigtx.
        #[ink(message)]
        pub fn check_signatures(
            &mut self,
        ) -> OtherResult<Vec<Signature>> {

            let thistime: Timestamp = self.env().block_timestamp();
            let caller: AccountID = AccountID { address: self.env().caller() };

            // make sure caller is designated multisigtx account
            if !self.multisig.signatories.contains(&caller) {

                return Err(Error::Custom(format!("CallerNotSignatory")));
            }

            // if multisigtx is too old, then it doesn't matter who signed
            if thistime - self.multisig.tx.time > self.multisig.timelimit {

                return Err(Error::Custom(format!("TransactionStale")));
            }

            Ok(self.multisig.tx.signatures.iter().map(|sig| *sig ).collect())
        }

////////////////////////////////////////////////////////////////////////////
/////// pausability ////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////

        /// - Function pauses contract.
        /// - Any signatory may call.
        #[ink(message)]
        pub fn pause(
            &mut self,
        ) -> OtherResult<()> {

            let caller: AccountID = AccountID { address: self.env().caller() };

            // make sure caller is designated multisigtx account
            if !self.multisig.signatories.contains(&caller) {

                return Err(Error::Custom(format!("CallerNotSignatory")));
            }

            self._pause()
        }

        /// - Function unpauses contract.
        #[ink(message)]
        pub fn unpause(
            &mut self,
            function: String,
        ) -> OtherResult<()> {
    
            // verify multisig good
            let _ = self.check_multisig(function)?;

            self._unpause()
        }

////////////////////////////////////////////////////////////////////////////
/////// minting ////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////

        /// - This generic mint function is for Art Zero interface.
        #[ink(message)]
        #[modifiers(only_owner)]
        #[openbrush::modifiers(when_not_paused)]
        pub fn mint(
            &mut self,
        ) -> Result<(), Error> {

            let caller = self.env().caller();

            // set next token id
            match self.last_token_id.checked_add(1) {
                Some(sum) => self.last_token_id = sum,
                None => return Err(Error::Custom(
                               format!("Overflow")))
            };

            // make sure cap is not surpassed
            if self.last_token_id >= self.access.cap {
                return Err(Error::Custom(
                       format!("The NFT cap of {:?} has been met. Cannot mint.", self.access.cap)))
            }

            // if cap not surpassed, mint next id
            let _ = self._mint_to(caller, Id::U64(self.last_token_id))?;

            // get nft collection of recipient if already holding
            let mut collection = match self.access.collections.get(caller) {
                Some(collection) => collection,
                None => Vec::new(),
            };

            // add id to recipient's nft collection
            collection.push(psp34::Id::U64(self.last_token_id));
            self.access.collections.insert(caller, &collection);

            Ok(())
        }

        /// - This mints a universal access nft by Interlock Network to specific recipient.
        #[ink(message)]
        #[modifiers(only_owner)]
        #[openbrush::modifiers(when_not_paused)]
        pub fn mint_to(
            &mut self,
            recipient: AccountId,
        ) -> Result<(), Error> {

            // set next token id
            match self.last_token_id.checked_add(1) {
                Some(sum) => self.last_token_id = sum,
                None => return Err(Error::Custom(
                               format!("Overflow")))
            };

            // make sure cap is not surpassed
            if self.last_token_id >= self.access.cap {
                return Err(Error::Custom(
                       format!("The NFT cap of {:?} has been met. Cannot mint.", self.access.cap)))
            }

            // if cap not surpassed, mint next id
            let _ = self._mint_to(recipient, psp34::Id::U64(self.last_token_id))?;

            // get nft collection of recipient if already holding
            let mut collection = match self.access.collections.get(recipient) {
                Some(collection) => collection,
                None => Vec::new(),
            };

            // add id to recipient's nft collection
            collection.push(psp34::Id::U64(self.last_token_id));
            self.access.collections.insert(recipient, &collection);

            Ok(())
        }

        /// - This mints a universal access nft to caller's self at token_price in terms of PSP22 token.
        #[ink(message)]
        #[openbrush::modifiers(when_not_paused)]
        pub fn self_mint(
            &mut self,
            price: Balance,
        ) -> Result<(), Error> {

            // set next token id
            match self.last_token_id.checked_add(1) {
                Some(sum) => self.last_token_id = sum,
                None => return Err(Error::Custom(
                               format!("Overflow")))
            };

            // mint recipient
            let minter: AccountId = self.env().caller();

            // make sure cap is not surpassed
            if self.last_token_id >= self.access.cap {
                return Err(Error::Custom(
                       format!("The NFT cap of {:?} has been met. Cannot mint.", self.access.cap)))
            }

            // make sure asking price matches nft_psp22price
            // ...this is to ensure that contract owner doesn't hike up token price between the
            //    time somebody checks the price, and the time that somebody submits tx to
            //    self-mint for that given price
            if self.access.nft_psp22price > price {
                return Err(Error::Custom(
                       format!("Current NFT price greater than agreed sale price of {:?}.",
                               self.access.nft_psp22price)))
            }

            // now connect to ilockmvp to transfer ILOCK of 'price' from minter to ilockmvp owner
            let _ = self.call_socket(minter, price, Vec::new())?;

            // mint next id
            let _ = self._mint_to(minter, psp34::Id::U64(self.last_token_id))?;

            // get nft collection of recipient if already holding
            let mut collection = match self.access.collections.get(minter) {
                Some(collection) => collection,
                None => Vec::new(),
            };

            // add id to recipient's nft collection
            collection.push(psp34::Id::U64(self.last_token_id));
            self.access.collections.insert(minter, &collection);

            Ok(())
        }

        /// - This is a mint function for Art Zero interface.
        #[ink(message)]
        #[modifiers(only_owner)]
        #[openbrush::modifiers(when_not_paused)]
        pub fn mint_with_attributes(
            &mut self,
            metadata: Vec<(String, String)>,
        ) -> Result<(), Error> {

            let caller = self.env().caller();

            // set next token id
            match self.last_token_id.checked_add(1) {
                Some(sum) => self.last_token_id = sum,
                None => return Err(Error::Custom(
                               format!("Overflow")))
            };

            // make sure cap is not surpassed
            if self.last_token_id >= self.access.cap {
                return Err(Error::Custom(
                       format!("The NFT cap of {:?} has been met. Cannot mint.", self.access.cap)))
            }

            // mint and set
            let _ = self._mint_to(caller, Id::U64(self.last_token_id))?;
            let _ = self.set_multiple_attributes(Id::U64(self.last_token_id), metadata)?;

            // update recipient's collection
            let mut collection = match self.access.collections.get(caller) {
                Some(collection) => collection,
                None => Vec::new(),
            };
            collection.push(Id::U64(self.last_token_id));
            self.access.collections.insert(caller, &collection);

            Ok(())
        }

        /// - This registers this universal access nft contract with
        /// ILOCK PSP22 token contract to allow self-minting.
        /// - Only contract owner may create a socket between this contract and the ILOCK PSP22 token.
        #[openbrush::modifiers(only_owner)]
        #[ink(message)]
        pub fn create_socket(
            &mut self
        ) -> Result<(), OtherError> {

            // make sure caller is operator
            if self.env().caller() != self.app.operator.address {

                return Err(OtherError::CallerNotOperator);
            }

            self.app.token_instance.create_socket(self.env().caller(), PORT)
        }

        /// - This makes call through universal access nft socket to ILOCK PSP22 token contract on
        /// port 0 or port 1, depending on this contract's configuration and affiliation with
        /// Interlock Network.
        /// - (Ie, transfer token from recipient to contract owner within PSP22 contract.)
        /// - Only operator may call.
        #[ink(message)]
        #[openbrush::modifiers(when_not_paused)]
        pub fn call_socket(
            &mut self,
            address: AccountId,
            amount: Balance,
            data: Vec<u8>,                  // <--! data vector to pass custom information to token
            ) -> Result<(), OtherError> {   //      contract logic

            self.app.token_instance.call_socket(address, amount, data)
        }

        /// - Retrieve the current price of universal access nft self-minting.
        #[ink(message)]
        pub fn get_token_price(
            &self,
        ) -> Balance {

            self.access.nft_psp22price
        }

        /// - Owner may change the price that self-minter must pay for universal access nft.
        #[openbrush::modifiers(only_owner)]
        #[ink(message)]
        pub fn set_token_price(
            &mut self,
            price: Balance,
        ) -> Result<(), PSP34Error> {

            self.access.nft_psp22price = price;

            Ok(())
        }

        /// - Get collection of nfts held by particular address.
        #[ink(message)]
        pub fn get_collection(
            &self,
            address: AccountId,
        ) -> Result<Vec<Id>, PSP34Error> {

            // retrieve the collection
            match self.access.collections.get(address) {
                Some(vec) => Ok(vec),
                None => Err(PSP34Error::Custom(
                        format!("The address {:?} does not have a collection.", address).into_bytes())),
            }
        }

        #[ink(message)]
        pub fn transfer_ownership(
            &mut self,
            newowner: AccountId,
            function: String,
        ) -> OtherResult<()> {
    
            // verify multisig good
            let _ = self.check_multisig(function)?;

            // make sure interlocker is not zero address
            if newowner == AccountId::from([0_u8; 32]) {

                return Err(Error::Custom(format!("IsZeroAddress")));
            }

            self.ownable.owner = newowner;

            Ok(())
        }

        /// - Modifies the code which is used to execute calls to this contract address.
        /// - This upgrades the token contract logic while using old state.
        #[ink(message)]
        pub fn update_contract(
            &mut self,
            code_hash: [u8; 32],
            function: String, 
        ) -> OtherResult<()> {
    
            // verify multisig good
            let _ = self.check_multisig(function)?;

            // takes code hash of updates contract and modifies preexisting logic to match
            ink::env::set_code_hash(&code_hash).unwrap_or_else(|err| {
                panic!(
                    "Failed to `set_code_hash` to {:?} due to {:?}",
                    code_hash, err
                )
            });

            Ok(())
        }

        /// - This is a testing helper for port applications.
        /// - Returns code hash for contract running e2e test (for port application verification).
        /// - Otherwise, each attempt to hardcode hash in application contract
        /// changes the hash for that run.
        #[ink(message)]
        pub fn contract_hash(
            &self,
            application: AccountId,
        ) -> Result<Hash, PSP34Error> {

            match self.env().code_hash(&application) {

                Ok(hash) => Ok(hash),
                Err(_error) => Err(PSP34Error::Custom(
                        format!("Not valid contract.").into_bytes())),
            }
        }

        /// - Art Zero helper function.
        pub fn add_attribute_name(
            &mut self,
            attribute_input: &Vec<u8>
        ) {
            let attr_input: String = String::from_utf8((*attribute_input).clone()).unwrap();
            let exist: bool = self.is_attribute.get(&attr_input).is_some();

            if !exist {
                self.attribute_count = self.attribute_count.checked_add(1).unwrap();
                self.attribute_names.insert(self.attribute_count, attribute_input);
                self.is_attribute.insert(&attr_input, &true);
            }
        }
    }

    /// - Art Zero traits implementation.
    /// - This is required to be commpatible with Art Zero Marketplace
    impl Psp34Traits for Psp34Nft {

        /// - Art Zero message.
        ///
        /// - Get total token count.
        #[ink(message)]
        fn get_last_token_id(
            &self
        ) -> u64 {

            return self.last_token_id;
        }

        /// - Art Zero message.
        ///
        /// - Lock UANFT, only token owner can call.
        #[ink(message)]
        fn lock(
            &mut self,
            token_id: Id
        ) -> Result<(), Error> {
            
            let caller = self.env().caller();

            let token_owner = match self.owner_of(token_id.clone()) {
                Some(owner) => owner,
                None => return Err(Error::Custom(
                        format!("Token does not exist."))),
            };

            if caller != token_owner {
                return Err(Error::Custom(
                        format!("Caller not token owner.")));
            }

            match self.locked_token_count.checked_add(1) {
                Some(sum) => self.locked_token_count = sum,
                None => return Err(Error::Custom(
                        format!("Overflow"))),
            };

            self.locked_tokens.insert(&token_id, &true);

            Ok(())
        }

        /// - Art Zero message.
        ///
        /// - Check if token is locked or not.
        #[ink(message)]
        fn is_locked_nft(
            &self,
            token_id: Id
        ) -> bool {

            match self.locked_tokens.get(&token_id) {
                Some(_) => return true,
                None => return false,
            }
        }

        /// - Art Zero message.
        ///
        /// - Get locked token count.
        #[ink(message)]
        fn get_locked_token_count(
            &self
        ) -> u64 {
            return self.locked_token_count;
        }

        /// - Art Zero message.
        ///
        /// - Change UANFT base URI.
        #[ink(message)]
        #[modifiers(only_owner)]
        fn set_base_uri(
            &mut self,
            uri: String
        ) -> Result<(), Error> {

            self._set_attribute(
                Id::U8(0),
                String::from("baseURI").into_bytes(),
                uri.into_bytes(),
            );
            Ok(())
        }

        /// - Art Zero message.
        ///
        /// - Only contract owner can set multiple attributes to a UANFT.
        #[ink(message)]
        #[modifiers(only_owner)]
        fn set_multiple_attributes(
            &mut self,
            token_id: Id,
            metadata: Vec<(String, String)>,
        ) -> Result<(), Error> {

            if token_id == Id::U64(0){
                return Err(Error::InvalidInput)
            }            
            if self.is_locked_nft(token_id.clone()) {
                return Err(Error::Custom(
                        String::from("Token is locked")));
            }
            for (attribute, value) in &metadata {
                self.add_attribute_name(&attribute.clone().into_bytes());
                self._set_attribute(token_id.clone(), attribute.clone().into_bytes(), value.clone().into_bytes());
            }

            Ok(())
        }

        /// - Art Zero message.
        ///
        /// - Get multiple attributes.
        #[ink(message)]
        fn get_attributes(
            &self,
            token_id: Id,
            attributes: Vec<String>
        ) -> Vec<String> {

            let length = attributes.len();
            let mut ret = Vec::<String>::new();
            for i in 0..length {
                let attribute = attributes[i].clone();
                let value = self.get_attribute(token_id.clone(), attribute.into_bytes());
                if value.is_some() {
                    ret.push(String::from_utf8(value.unwrap()).unwrap());
                } else {
                    ret.push(String::from(""));
                }
            }
            ret
        }

        /// - Art Zero message.
        ///
        /// - Get attribute count.
        #[ink(message)]
        fn get_attribute_count(
            &self
        ) -> u32 {
            self.attribute_count
        }

        /// - Art Zero message.
        ///
        /// - Get attribute name.
        #[ink(message)]
        fn get_attribute_name(
            &self,
            index: u32
        ) -> String {
            
            match self.attribute_names.get(&index) {
                Some(attribute) => String::from_utf8(attribute).unwrap(),
                None => String::from(""),
            }
        }

        /// - Art Zero message.
        ///
        /// - Get URI from UANFT Id.
        #[ink(message)]
        fn token_uri(
            &self,
            token_id: u64
        ) -> String {
            let value = self.get_attribute(Id::U8(0), String::from("baseURI").into_bytes());
            let mut token_uri = String::from_utf8(value.unwrap()).unwrap();
            token_uri = token_uri + &token_id.to_string() + &String::from(".json");
            token_uri
        }

        /// - Art Zero message.
        ///
        /// - Get contract owner.
        #[ink(message)]
        fn get_owner(
            &self,
        ) -> AccountId {
            
            self.owner()
        }
    }
}

#[cfg(all(test, feature = "e2e-tests"))]
pub mod tests_e2e;

//
// TEST TODO
// in order of appearance
//
// [x] happye2e_transfer      
// [] sade2e_transfer       
// [x] happyunit_new (no sad, returns only Self)
// [x] happye2e_mint
//      [x] happye2e_get_collection
// [] sade2e_mint
// [x] happye2e_self_mint            <-- includes call_socket()
// [] sade2e_self_mint
// [x] ** happye2e_create_socket
// [] ** sade2e_create_socket 
// [x] ** happye2e_call_socket
// [] ** sade2e_call_socket
// [x] happyunit_register
//      [x] happyunit_set_credential
//      [x] happyunit_get_gredental
//      [x] happyunit_is_authenticated
//      [x] happyunit_revoke_access
// [x] happyunit_set_token_price
//      [x] happyunit_get_token_price
//
