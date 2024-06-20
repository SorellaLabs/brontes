#!/bin/bash

# Generate Tarball of database
tar -czvf ~/brontes-db-latest.tar.gz $1
# get byte size and write to folder
wc -c ~/brontes-db-latest.tar.gz  | awk '{print $1}' | tr -d '\n' > ~/db-size.txt
# upload db to r2
rclone copy ~/brontes-db-latest.tar.gz r2:brontes-db-latest/ --s3-upload-cutoff=100M --s3-chunk-size=100M
# upload byte-count to r2
rclone copy ~/db-size.txt r2:brontes-db-latest/
