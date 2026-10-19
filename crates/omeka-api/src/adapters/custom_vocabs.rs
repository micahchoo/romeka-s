//! `CustomVocabAdapter` registration (Custom Vocab module v2.1.0,
//! `config/module.config.php` `api_adapters`). The port builds the module
//! into core, so the term lists are a compiled-in adapter.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `CustomVocab\Api\Adapter\CustomVocabAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "custom_vocabs",
    representation: "CustomVocab\\Api\\Representation\\CustomVocabRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
