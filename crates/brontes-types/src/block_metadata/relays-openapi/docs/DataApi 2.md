# \DataApi

All URIs are relative to *http://localhost:18550*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_delivered_payloads**](DataApi.md#get_delivered_payloads) | **GET** /relay/v1/data/bidtraces/proposer_payload_delivered | Get payloads that were delivered to proposers.
[**get_received_bids**](DataApi.md#get_received_bids) | **GET** /relay/v1/data/bidtraces/builder_blocks_received | Get builder bid submissions.
[**get_validator_registration**](DataApi.md#get_validator_registration) | **GET** /relay/v1/data/validator_registration | Check that a validator is registered with the relay.



## get_delivered_payloads

> Vec<crate::models::GetDeliveredPayloads200ResponseInner> get_delivered_payloads(slot, cursor, limit, block_hash, block_number, proposer_pubkey, builder_pubkey, order_by)
Get payloads that were delivered to proposers.

* Payloads become available after the relay responds to a `getPayload` request from the proposer.  * Query arguments are used as filters. 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**slot** | Option<**String**> | A specific slot. |  |
**cursor** | Option<**String**> | A starting slot for multiple results. |  |
**limit** | Option<**String**> | The number of results. |  |
**block_hash** | Option<**String**> | A specific block hash. |  |
**block_number** | Option<**String**> | A specific block number. |  |
**proposer_pubkey** | Option<**String**> | A specific proposer public key. |  |
**builder_pubkey** | Option<**String**> | A specific builder public key. |  |
**order_by** | Option<**String**> | Sort results in order of... |  |

### Return type

[**Vec<crate::models::GetDeliveredPayloads200ResponseInner>**](getDeliveredPayloads_200_response_inner.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_received_bids

> Vec<crate::models::GetReceivedBids200ResponseInner> get_received_bids(slot, block_hash, block_number, builder_pubkey, limit)
Get builder bid submissions.

* Returns a list of builder bids without execution payloads.  * Only submissions that were successfully verified. 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**slot** | Option<**String**> | A specific slot. |  |
**block_hash** | Option<**String**> | A specific block hash. |  |
**block_number** | Option<**String**> | A specific block number. |  |
**builder_pubkey** | Option<**String**> | A specific builder public key. |  |
**limit** | Option<**String**> | The number of results. |  |

### Return type

[**Vec<crate::models::GetReceivedBids200ResponseInner>**](getReceivedBids_200_response_inner.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_validator_registration

> crate::models::GetValidators200ResponseInnerEntry get_validator_registration(pubkey)
Check that a validator is registered with the relay.

* Returns the latest validator registration for a given pubkey.  * Useful to check whether your own registration was successful. 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**pubkey** | **String** | The validator's public key. | [required] |

### Return type

[**crate::models::GetValidators200ResponseInnerEntry**](getValidators_200_response_inner_entry.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

