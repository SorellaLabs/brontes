SELECT
    toString(address) AS address,
    toString(classifier_name) as classifier_name
FROM brontes.protocol_details
FINAL
WHERE classifier_name IS NOT NULL AND classifier_name != ''

-- ok