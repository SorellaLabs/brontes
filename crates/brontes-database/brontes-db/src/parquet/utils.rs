use std::{fmt::Display, sync::Arc};

use arrow::{
    array::{
        Array, BinaryArray, BinaryBuilder, Float64Array, Float64Builder, ListArray, ListBuilder,
        StringArray, StringBuilder, UInt64Array,
    },
    datatypes::Schema,
    error::ArrowError,
    record_batch::RecordBatch,
};

pub fn u128_to_binary_array(values: Vec<u128>) -> BinaryArray {
    let data_capacity = values.len() * 16;
    let mut builder = BinaryBuilder::with_capacity(values.len(), data_capacity);
    for value in values {
        let bytes = value.to_be_bytes();
        builder.append_value(bytes);
    }
    builder.finish()
}

pub fn build_string_array(values: Vec<String>) -> StringArray {
    StringArray::from_iter_values(values)
}

pub fn get_string_array(values: Vec<Option<&str>>) -> StringArray {
    let mut builder = StringBuilder::new();
    for value in values {
        builder.append_option(value);
    }
    builder.finish()
}

pub fn get_string_array_from_owned<S: AsRef<str>>(values: Vec<Option<S>>) -> StringArray {
    let mut builder = StringBuilder::new();
    for value in values {
        builder.append_option(value);
    }
    builder.finish()
}

pub fn get_list_string_array<Q>(values: Vec<&Vec<Q>>) -> ListArray
where
    Q: Display + AsRef<str>,
{
    let mut builder = ListBuilder::new(StringBuilder::new());

    for v in values {
        let string_builder = builder.values();
        if v.is_empty() {
            builder.append_null();
            continue;
        } else {
            for label in v {
                string_builder.append_value(label);
            }
            builder.append(true)
        }
    }

    builder.finish()
}

pub fn get_list_string_array_from_owned<Q>(values: Vec<Vec<Q>>) -> ListArray
where
    Q: Display + AsRef<str>,
{
    let mut builder = ListBuilder::new(StringBuilder::new());

    for v in values {
        let string_builder = builder.values();
        if v.is_empty() {
            builder.append_null();
            continue;
        } else {
            for label in v {
                string_builder.append_value(label);
            }
            builder.append(true)
        }
    }

    builder.finish()
}

pub fn get_list_float_array_from_owned(values: Vec<Vec<f64>>) -> ListArray {
    let mut builder = ListBuilder::new(Float64Builder::new());

    for v in values {
        let float_builder = builder.values();
        if v.is_empty() {
            builder.append_null();
            continue;
        } else {
            for float in v {
                float_builder.append_value(float);
            }
            builder.append(true)
        }
    }

    builder.finish()
}

pub fn build_uint64_array(values: Vec<u64>) -> UInt64Array {
    UInt64Array::from(values)
}

pub fn build_float64_array(values: Vec<f64>) -> Float64Array {
    Float64Array::from(values)
}

pub fn build_record_batch(
    schema: Schema,
    arrays: Vec<Arc<dyn Array>>,
) -> Result<RecordBatch, ArrowError> {
    RecordBatch::try_new(Arc::new(schema), arrays)
}
