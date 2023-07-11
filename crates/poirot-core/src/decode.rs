use alloy_dyn_abi::DynSolType;
use alloy_json_abi::JsonAbi;
use reth_rpc_types::trace::parity::{Action as RethAction, LocalizedTransactionTrace};
use revm_primitives::bits::B160;
use std::{collections::HashMap, path::PathBuf};

pub struct ContractAbiStorage<'a> {
    mapping: HashMap<&'a B160, PathBuf>,
}

// TODO: I need you to write the etherscan api call to get the abi for a contract instead of the
// TODO: hashmap we have here, i added the api key to the .env file


impl<'a> ContractAbiStorage<'a> {
    pub fn new() -> Self {
        Self { mapping: HashMap::new() }
    }

    pub fn add_abi(&mut self, contract_address: &'a B160, abi_path: PathBuf) {
        self.mapping.insert(contract_address, abi_path);
    }

    pub fn get_abi(&self, contract_address: &'a B160) -> Option<&PathBuf> {
        self.mapping.get(contract_address)
    }
}

pub fn sleuth<'a>(
    storage: &'a ContractAbiStorage,
    trace: LocalizedTransactionTrace,
) -> Result<String, Box<dyn std::error::Error>> {
    let action = trace.trace.action;

    let (contract_address, input) = match action {
        RethAction::Call(call_action) => (call_action.to, call_action.input.to_vec()),
        _ => return Err(From::from("The action in the transaction trace is not Call(CallAction)")),
    };

    let abi_path = storage.get_abi(&contract_address).ok_or("No ABI found for this contract")?;

    let file = std::fs::File::open(abi_path)?;
    let reader = std::io::BufReader::new(file);

    let json_abi: JsonAbi = serde_json::from_reader(reader)?;

    let function_selector = &input[..4];

    //todo:
    if let Some(functions) = Some(json_abi.functions.values().flatten()) {
        for function in functions {
            if function.selector() == function_selector {
                let input_types: Vec<String> =
                    function.inputs.iter().map(|input| input.to_string()).collect();

                let mut decoded_inputs = Vec::new();
                for (index, input_type_str) in input_types.iter().enumerate() {
                    //TODO: just fix this, where you properly provide the expected decoding from
                    // the abi
                    let input_data = &input[4 + index..]; // Skip the function selector and previous inputs
                    let dyn_sol_type: DynSolType = input_type_str.parse().unwrap();
                    let dyn_sol_value = dyn_sol_type.decode_params(input_data)?;
                    decoded_inputs.push(format!("{:?}", dyn_sol_value));
                }

                let printout = format!("Function: {}\nInputs: {:?}", function.name, decoded_inputs);
                return Ok(printout)
            }
        }
    }

    Err(From::from("No matching function found in the ABI"))
}



// TODO: Add tests! 