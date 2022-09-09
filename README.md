# Schema Rustgistry
☕️
````
# Register a new schema
curl localhost:8080/subjects/subject1/versions -X POST -d '{"schema": "[\"string\"]"}' -H "Content-Type: application/json"
{"id":1}

# Send twice, confirm same id
curl localhost:8080/subjects/subject1/versions -X POST -d '{"schema": "[\"string\"]"}' -H "Content-Type: application/json"
{"id":1}

# Confirm no new version
curl localhost:8080/subjects/subject1/versions                                                                            
[1]

# Register same schema on a new subject name
curl localhost:8080/subjects/subject2/versions -X POST -d '{"schema": "[\"string\"]"}' -H "Content-Type: application/json"
{"id":1}

# Register a new schema
curl localhost:8080/subjects/subject2/versions -X POST -d '{"schema": "[\"long\"]"}' -H "Content-Type: application/json"
{"id":2}

# Confirm version 1 and version 2
curl localhost:8080/subjects/subject2/versions/2
{"id":1,"name":"subject2","version":1,"schema":"[\"string\"]"} 

curl localhost:8080/subjects/blublu2/versions/2
{"id":2,"name":"subject2","version":2,"schema":"[\"long\"]"} 

# Get schema
curl localhost:8080/subjects/blublu2/versions/2/schema
["long"]
````