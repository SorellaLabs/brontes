#!/bin/bash

TIMESTAMP=`date +%s`
# Generate Tarball of database
tar -czvf ~/brontes-db-$TIMESTAMP.tar.gz $1
# post data to url 
curl -X post --data-binary @~/brontes-db-$TIMESTAMP.tar.gz $2
