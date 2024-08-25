# \BuilderApi

All URIs are relative to *http://localhost:18550*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_validators**](BuilderApi.md#get_validators) | **GET** /relay/v1/builder/validators | Get a list of validator registrations for validators scheduled to propose in the current and next epoch. 
[**submit_block**](BuilderApi.md#submit_block) | **POST** /relay/v1/builder/blocks | Submit a new block to the relay.



## get_validators

> Vec<crate::models::GetValidators200ResponseInner> get_validators()
Get a list of validator registrations for validators scheduled to propose in the current and next epoch. 

* Used by builders to know when to submit bids for an upcoming proposal.  * Returns an array of validator registrations for the current and next epoch.  * Each entry includes a slot and the validator with assigned duty.  * Slots without a registered validator are omitted. 

### Parameters

This endpoint does not need any parameter.

### Return type

[**Vec<crate::models::GetValidators200ResponseInner>**](getValidators_200_response_inner.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## submit_block

> crate::models::SubmitBlock200Response submit_block(submit_block_request, cancellations)
Submit a new block to the relay.

* Blocks can be submitted as JSON or SSZ, and optionally GZIP encoded. To be   clear, there are four options: JSON, JSON+GZIP, SSZ, SSZ+GZIP. If JSON, the   content type should be `application/json`. If SSZ, the content type should   be `application/octet-stream`.  * The relay will simulate the block to verify properties and proposer   payment in the payment transaction from builder to proposer   `fee_recipient` at the end of block.  * For accountability, builder signature is over the SSZ encoded `message`.  * The `message`, which does not include the transactions, will be made   public via the data API, allowing anyone to verify the builder signature.  * Any new submission by a builder will overwrite a previous one by the same   `builder_pubkey`, even if it is less profitable. 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**submit_block_request** | [**SubmitBlockRequest**](SubmitBlockRequest.md) | A signed bid with an execution payload. | [required] |
**cancellations** | Option<**String**> | If set to 1, opt into bid cancellations. |  |

### Return type

[**crate::models::SubmitBlock200Response**](submitBlock_200_response.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json, application/octet-stream
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

