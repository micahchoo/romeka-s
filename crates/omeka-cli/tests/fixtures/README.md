# Fixtures

`oracle-default-vocabularies.json` is a fresh upstream install (the oracle on port 8899) read back
through its API: each default vocabulary with its classes and properties in id order, as
`InstallDefaultVocabulariesTask` stored them. `tests/default_vocabularies.rs` compares
`romeka install` against it. It is also the English of the vocabulary label and comment tables
(`web/admin/scripts/extract-vocabulary-text.mjs`, checked by `omeka-site`'s vocabulary tests), so a
recapture that changes a text means re-running that script and the translations. To read it again:

```
B=http://localhost:8899/api
for v in 1 2 3 4; do
  jq -n --argjson voc "$(curl -s $B/vocabularies/$v)" \
    --argjson c "$(curl -s "$B/resource_classes?vocabulary_id=$v&per_page=1000&sort_by=id")" \
    --argjson p "$(curl -s "$B/properties?vocabulary_id=$v&per_page=1000&sort_by=id")" \
    '{prefix: $voc["o:prefix"], namespace_uri: $voc["o:namespace_uri"], label: $voc["o:label"],
      comment: $voc["o:comment"],
      classes: [$c[] | {local_name: .["o:local_name"], label: .["o:label"], comment: .["o:comment"]}],
      properties: [$p[] | {local_name: .["o:local_name"], label: .["o:label"], comment: .["o:comment"]}]}'
done | jq -s . > oracle-default-vocabularies.json
```
