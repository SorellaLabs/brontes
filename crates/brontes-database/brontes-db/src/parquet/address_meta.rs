use std::sync::Arc;

use alloy_primitives::Address;
use arrow::{
    array::{
        Array, ArrayRef, BooleanBuilder, StringBuilder, StructArray, UInt64Builder, UInt8Builder,
    },
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::db::address_metadata::AddressMetadata;
use itertools::Itertools;

use super::utils::{build_string_array, get_list_string_array, get_string_array};

pub fn address_metadata_to_record_batch(
    address_metadata: Vec<(Address, AddressMetadata)>,
) -> Result<RecordBatch, ArrowError> {
    let address_array = build_string_array(
        address_metadata
            .iter()
            .map(|am| am.0.to_string())
            .collect_vec(),
    );

    let entity_name_array = get_string_array(
        address_metadata
            .iter()
            .map(|am| am.1.entity_name.as_deref())
            .collect_vec(),
    );
    let nametag_array = get_string_array(
        address_metadata
            .iter()
            .map(|am| am.1.nametag.as_deref())
            .collect_vec(),
    );

    let labels_array =
        get_list_string_array(address_metadata.iter().map(|am| &am.1.labels).collect_vec());

    let address_type_array = get_string_array(
        address_metadata
            .iter()
            .map(|am| am.1.address_type.as_deref())
            .collect_vec(),
    );
    let ens_array = get_string_array(
        address_metadata
            .iter()
            .map(|am| am.1.ens.as_deref())
            .collect_vec(),
    );

    let contract_info_array =
        get_contract_info_array(address_metadata.iter().map(|am| &am.1).collect_vec());
    let socials_array = get_socials_array(address_metadata.iter().map(|am| &am.1).collect_vec());

    let schema = Schema::new(vec![
        Field::new("address", DataType::Utf8, false),
        Field::new("entity_name", DataType::Utf8, true),
        Field::new("nametag", DataType::Utf8, true),
        Field::new(
            "labels",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("address_type", DataType::Utf8, true),
        Field::new("ens", DataType::Utf8, true),
        Field::new("contract_info", contract_info_array.data_type().clone(), true),
        Field::new("social_metadata", socials_array.data_type().clone(), true),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(address_array),
            Arc::new(entity_name_array),
            Arc::new(nametag_array),
            Arc::new(labels_array),
            Arc::new(address_type_array),
            Arc::new(ens_array),
            Arc::new(contract_info_array),
            Arc::new(socials_array),
        ],
    )
}

fn get_contract_info_array(address_metadata: Vec<&AddressMetadata>) -> StructArray {
    let mut verified_contract_builder = BooleanBuilder::new();
    let mut contract_creator_builder = StringBuilder::new();
    let mut reputation_builder = UInt8Builder::new();

    for meta in address_metadata {
        if let Some(contract_info) = &meta.contract_info {
            verified_contract_builder.append_option(contract_info.verified_contract);
            contract_creator_builder.append_option(
                contract_info
                    .contract_creator
                    .as_ref()
                    .map(|addr| addr.to_string()),
            );
            reputation_builder.append_option(contract_info.reputation);
        } else {
            verified_contract_builder.append_null();
            contract_creator_builder.append_null();
            reputation_builder.append_null();
        }
    }

    let verified_contract_array = verified_contract_builder.finish();
    let contract_creator_array = contract_creator_builder.finish();
    let reputation_array = reputation_builder.finish();

    let fields = vec![
        Field::new("verified_contract", DataType::Boolean, true),
        Field::new("contract_creator", DataType::Utf8, true),
        Field::new("reputation", DataType::UInt8, true),
    ];

    let arrays = vec![
        Arc::new(verified_contract_array) as ArrayRef,
        Arc::new(contract_creator_array) as ArrayRef,
        Arc::new(reputation_array) as ArrayRef,
    ];

    StructArray::try_new(fields.into(), arrays, None).expect("Failed to init struct arrays")
}

fn get_socials_array(address_metadata: Vec<&AddressMetadata>) -> StructArray {
    let mut twitter_builder = StringBuilder::new();
    let mut twitter_followers_builder = UInt64Builder::new();
    let mut website_url_builder = StringBuilder::new();
    let mut crunchbase_builder = StringBuilder::new();
    let mut linkedin_builder = StringBuilder::new();

    for meta in address_metadata {
        twitter_builder.append_option(meta.social_metadata.twitter.as_deref());
        twitter_followers_builder.append_option(meta.social_metadata.twitter_followers);
        website_url_builder.append_option(meta.social_metadata.website_url.as_deref());
        crunchbase_builder.append_option(meta.social_metadata.crunchbase.as_deref());
        linkedin_builder.append_option(meta.social_metadata.linkedin.as_deref());
    }

    let twitter_array = twitter_builder.finish();
    let twitter_followers_array = twitter_followers_builder.finish();
    let website_url_array = website_url_builder.finish();
    let crunchbase_array = crunchbase_builder.finish();
    let linkedin_array = linkedin_builder.finish();

    let fields = vec![
        Field::new("twitter", DataType::Utf8, true),
        Field::new("twitter_followers", DataType::UInt64, true),
        Field::new("website_url", DataType::Utf8, true),
        Field::new("crunchbase", DataType::Utf8, true),
        Field::new("linkedin", DataType::Utf8, true),
    ];

    let arrays = vec![
        Arc::new(twitter_array) as ArrayRef,
        Arc::new(twitter_followers_array) as ArrayRef,
        Arc::new(website_url_array) as ArrayRef,
        Arc::new(crunchbase_array) as ArrayRef,
        Arc::new(linkedin_array) as ArrayRef,
    ];

    StructArray::try_new(fields.into(), arrays, None).expect("Failed to init struct arrays")
}
