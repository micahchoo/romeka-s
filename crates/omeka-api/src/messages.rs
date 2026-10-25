//! Every English message the API and the server put in an error document.
//!
//! The English text is upstream's msgid: the admin translates a message the
//! way gettext does, by looking the English pattern up in its catalogue. So
//! each pattern has one home, this registry, and every construction site calls
//! it instead of writing the text. A message that substitutes values is a
//! function whose argument names are the pattern's `{name}` holes; `format!`
//! checks at compile time that the holes and the arguments agree.
//!
//! `web/admin/src/generated/serverMessages.json` is this registry, keyed by
//! catalogue key. The test at the bottom fails when the file is stale, and a
//! catalogue that lacks a key is then a `tsc` error in the admin.
//!
//! The wire stays byte-for-byte upstream's text. A free-form message (a
//! storage failure, a `serde_json` parse error) is not a pattern and reaches
//! the admin as it is.

/// One registered pattern: its Rust name and its English text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerMessage {
    /// The constant or function name the pattern is registered under.
    pub name: &'static str,
    /// The English text, with each substituted value as a `{name}` hole.
    pub pattern: &'static str,
}

impl ServerMessage {
    /// The admin catalogue key: `server.` and the name in camel case.
    #[must_use]
    pub fn key(&self) -> String {
        let mut key = String::from("server.");
        let mut upper = false;
        for ch in self.name.chars() {
            if ch == '_' {
                upper = true;
            } else if upper {
                key.extend(ch.to_uppercase());
                upper = false;
            } else {
                key.extend(ch.to_lowercase());
            }
        }
        key
    }
}

/// One registry item: a `const` for a fixed text, a `fn` for a pattern with
/// holes.
macro_rules! message_item {
    ($(#[$doc:meta])* const $name:ident [] $pattern:literal) => {
        $(#[$doc])*
        pub const $name: &str = $pattern;
    };
    ($(#[$doc:meta])* fn $name:ident [$($arg:ident),+] $pattern:literal) => {
        $(#[$doc])*
        #[must_use]
        pub fn $name($($arg: impl std::fmt::Display),+) -> String {
            format!($pattern, $($arg = $arg),+)
        }
    };
}

/// The registry: each item, and [`REGISTRY`] listing them all.
macro_rules! messages {
    ($(
        $(#[$doc:meta])*
        $kind:ident $name:ident $(($($arg:ident),+))? = $pattern:literal;
    )*) => {
        $(message_item!($(#[$doc])* $kind $name [$($($arg),+)?] $pattern);)*

        /// Every registered pattern, in source order.
        pub const REGISTRY: &[ServerMessage] = &[
            $(ServerMessage { name: stringify!($name), pattern: $pattern },)*
        ];
    };
}

messages! {
    // Routing and the generic error documents.

    /// Laminas' text for a 405 response body.
    const METHOD_NOT_ALLOWED = "Method Not Allowed";
    /// The 404 body for a path no route claims.
    const NOT_FOUND = "Not Found";
    /// The 400 body a body-limit failure carries.
    const BAD_REQUEST = "Bad Request";
    /// The 408 body for a request past the configured timeout.
    const REQUEST_TIMEOUT = "Request Timeout";
    /// The 413 body for a request over the body limit.
    const BODY_TOO_LARGE = "Request body too large";
    /// The 500 body every internal failure answers with on the account routes.
    const INTERNAL_ERROR = "Internal Server Error";
    /// The 429 body for a login over the attempt limit.
    const TOO_MANY_REQUESTS = "Too Many Requests";
    /// `Api\Manager::execute` for a resource name no adapter serves.
    fn unknown_resource(resource) = "The API does not support the \"{resource}\" resource.";
    /// Upstream's `OperationNotImplementedException` sentence, with the
    /// resource name in place of the PHP class name.
    fn adapter_not_implemented(resource, operation) =
        "The {resource} adapter does not implement the {operation} operation.";
    /// The port's 501 for a route that answers without an adapter.
    fn endpoint_not_implemented(endpoint) = "The {endpoint} endpoint is not implemented yet.";
    /// `Api\Manager::execute` and `AbstractEntityAdapter::authorize`.
    fn permission_denied(operation, resource) =
        "Permission denied for the current user to {operation} the {resource} resource.";
    /// `AbstractEntityAdapter::findEntity`; `criteria` is the JSON upstream
    /// `json_encode`s.
    fn entity_not_found(class, criteria) = "{class} entity with criteria {criteria} not found";

    // The request body.

    /// `ApiController` for a body that is not JSON.
    const JSON_SYNTAX = "JSON: Syntax error";
    /// `ApiController` for a JSON body that is a scalar.
    const JSON_NOT_CONTAINER = "JSON: Content must be an object or array.";
    /// `ApiController::checkContentType` for a write that is not JSON or
    /// multipart.
    fn invalid_content_type(received) =
        "Invalid Content-Type header. Expecting \"application/json\", got \"{received}\".";
    /// An account route whose body is not a JSON object.
    const BODY_NOT_OBJECT = "The request body must be a JSON object.";
    /// The site items route for a body without exactly one selector.
    const SITE_ITEMS_ONE_SELECTOR = "name either \"query\", or \"items\", \"item_sets\" or both";

    // Sign-in, sessions and passwords.

    /// `LoginController::loginAction` for a wrong email or password.
    const INVALID_CREDENTIALS = "Email or password is invalid";
    /// `LoginController::loginAction` for an inactive user.
    const INACTIVE_ACCOUNT = "The user is not active.";
    /// `LoginController::createPasswordAction` for an unknown or expired key.
    const UNKNOWN_KEY = "Invalid password creation key.";
    /// The account routes for a request without a session.
    const AUTHENTICATION_REQUIRED = "Authentication required.";
    /// `PasswordConfirm::passwordIsValid`.
    const INVALID_PASSWORD = "The password is invalid";
    /// A session-authenticated write from another origin.
    const CROSS_ORIGIN_REFUSED =
        "A session-authenticated write must come from the configured base URL.";

    // Resources and values.

    /// `ValueHydrator` for a term the catalog does not know.
    fn property_not_recognized(term) = "The property \"{term}\" is not recognized.";
    /// `ResourceAdapter` for a value resource that does not exist.
    fn resource_not_found(id) = "Resource not found with id {id}.";
    /// `AbstractResourceEntityAdapter` for a required template property.
    fn template_requires_value(template, property) =
        "The \"{template}\" resource template requires a \"{property}\" value";
    /// `ItemAdapter` for a non-array `o:item_set`.
    const ITEM_SETS_NOT_ARRAY = "Item sets must be an array";
    /// `ItemAdapter` for a non-array `o:media`.
    const MEDIA_NOT_ARRAY = "Media must be an array";
    /// `ItemAdapter` for a media entry of another item.
    const MEDIA_NOT_THIS_ITEM = "Media must belong to this item.";
    /// `ItemAdapter` for a primary media of another item.
    const PRIMARY_MEDIA_NOT_THIS_ITEM = "Primary media must belong to this item.";
    /// `MediaAdapter` for a media without an ingester.
    const MEDIA_INGESTER_REQUIRED = "Media must set an ingester.";
    /// `MediaAdapter` for an ingester no module registers.
    const MEDIA_INGESTER_INVALID = "Media must set a valid ingester.";
    /// `MediaAdapter` for a media without an item.
    const MEDIA_ITEM_REQUIRED = "Media must belong to an item.";

    // Jobs.

    /// The 409 for deleting the job of an archive import that is running or undoing.
    const JOB_HELD_BY_IMPORT = "This job belongs to an archive import that is running or undoing. Delete it once the import has ended.";
    /// The 409 for a site page save whose `o:modified` is older than the stored page's.
    const PAGE_SAVED_SINCE_OPENED = "Someone saved this page after you opened it.";

    // Assets.

    /// `AssetAdapter` for a create without a file.
    const NO_FILE_UPLOADED = "No file was uploaded";
    /// `Validator::validate` for a media type outside the allow list.
    fn asset_media_type(file, media) =
        "Error validating \"{file}\". Cannot store files with the media type \"{media}\".";
    /// `Validator::validate` for an extension outside the allow list.
    fn asset_extension(file, extension) =
        "Error validating \"{file}\". Cannot store files with the resolved extension \"{extension}\".";
    /// `AssetAdapter` for an empty name.
    const ASSET_NAME_REQUIRED = "An asset must have a name.";

    // Vocabularies, classes, properties and templates.

    /// `VocabularyAdapter` for a non-array `o:class`.
    const CLASSES_NOT_ARRAY = "Classes must be an array";
    /// `VocabularyAdapter` for a non-array `o:property`.
    const PROPERTIES_NOT_ARRAY = "Properties must be an array";
    /// `VocabularyAdapter` for an empty namespace.
    const NAMESPACE_URI_EMPTY = "The namespace URI cannot be empty.";
    /// `VocabularyAdapter` for a namespace another vocabulary holds.
    fn namespace_uri_taken(uri) = "The namespace URI \"{uri}\" is already taken.";
    /// `VocabularyAdapter` for an empty prefix.
    const PREFIX_EMPTY = "The prefix cannot be empty.";
    /// `VocabularyAdapter` for a prefix outside the allowed characters.
    const PREFIX_INVALID = "The prefix may only contain letters, numbers, hyphens, and underscores.";
    /// `VocabularyAdapter` for a prefix another vocabulary holds.
    fn prefix_taken(prefix) = "The prefix \"{prefix}\" is already taken.";
    /// `VocabularyAdapter` for a reserved prefix.
    fn prefix_reserved(prefix) = "The prefix \"{prefix}\" is reserved.";
    /// The vocabulary, member and template adapters for an empty label.
    const LABEL_EMPTY = "The label cannot be empty.";
    /// `ResourceTemplateAdapter` for a label another template holds.
    const LABEL_TAKEN = "The label is already taken.";
    /// The member adapters for an empty local name.
    const LOCAL_NAME_EMPTY = "The local name cannot be empty.";
    /// The member adapters and `VocabularyAdapter` for a local name in use.
    fn local_name_taken(name) = "The local name \"{name}\" is already taken.";
    /// The member adapters for a member without a vocabulary.
    const VOCABULARY_REQUIRED = "A vocabulary must be set.";
    /// `ResourceTemplateAdapter` for a property listed twice.
    fn duplicate_template_property(id) = "Attempting to add duplicate property with ID {id}";

    // Vocabulary import (`Stdlib/RdfImporter`, `Admin/VocabularyController`).

    /// `RdfImporter::getMembers` for the file strategy without a file.
    const VOCABULARY_NO_FILE = "No file specified for the file import strategy.";
    /// `RdfImporter::getMembers` for the URL strategy without a URL.
    const VOCABULARY_NO_URL = "No URL specified for the URL import strategy.";
    /// `RdfImporter::getMembers` for a file EasyRdf cannot parse.
    const VOCABULARY_NOT_PARSED = "Could not parse vocabulary file.";
    /// `RdfImporter::getMembers` for a URL EasyRdf cannot load.
    const VOCABULARY_NOT_LOADED = "Could not load vocabulary from URL.";
    /// `RdfImporter::import` for a file with no member in the namespace.
    const VOCABULARY_NO_MEMBERS = "No classes or properties found. Are you sure you used the correct namespace URI?";
    /// `VocabularyController::editAction` for Dublin Core and Dublin Core Type.
    const VOCABULARY_PERMANENT = "Cannot edit a permanent vocabulary";
    /// The import's format detection found no format it reads.
    const VOCABULARY_FORMAT_UNKNOWN = "The file format could not be detected. Choose the format from the list.";
    /// The import's `format` names no format it reads.
    fn vocabulary_format_invalid(format) = "\"{format}\" is not a vocabulary file format.";
    /// The import's URL fetch for a scheme other than http and https.
    const VOCABULARY_URL_SCHEME = "Only http and https URLs can be imported.";
    /// The import's URL fetch for a private or local address.
    const VOCABULARY_URL_PRIVATE = "The URL names a private or local network address, which the server does not fetch.";
    /// The import's URL fetch for a file over the cap.
    fn vocabulary_url_too_large(size) = "The file at the URL is larger than {size}.";
    /// The import's URL fetch past its deadline.
    const VOCABULARY_URL_TIMEOUT = "The server at the URL did not answer in time.";
    /// The import's URL fetch for an error status.
    fn vocabulary_url_status(status) = "The server at the URL answered with status {status}.";
    /// A reviewed change whose stored text changed after the comparison.
    fn vocabulary_change_stale(name) = "\"{name}\" has changed since the comparison. Compare the file again.";
    /// A reviewed change to a member the vocabulary does not have.
    fn vocabulary_member_missing(name) = "\"{name}\" is not a member of this vocabulary.";

    // Users.

    /// `UserAdapter` for an empty name.
    const NAME_EMPTY = "The name cannot be empty.";
    /// `UserAdapter` for an email another user holds.
    fn email_taken(email) = "The email {email} is already taken.";
    /// `UserAdapter` for a user without a role.
    const USER_ROLE_REQUIRED = "Users must have a role.";
    /// Laminas `EmailAddress::INVALID_FORMAT`.
    const INVALID_EMAIL =
        "The input is not a valid email address. Use the basic format local-part@hostname";

    // Sites, pages and navigation.

    /// `SiteAdapter` for an empty title.
    const SITE_TITLE_REQUIRED = "A site must have a title.";
    /// `SiteAdapter` for a site without a theme.
    const SITE_THEME_REQUIRED = "A site must have a theme.";
    /// `SiteAdapter` for a site without item pool data.
    const SITE_ITEM_POOL_REQUIRED = "A site must have item pool data.";
    /// `SiteAdapter` for a homepage of another site.
    const HOMEPAGE_NOT_IN_SITE = "A homepage must belong to its parent site.";
    /// The site and page adapters for an empty slug.
    const SLUG_EMPTY = "The slug cannot be empty.";
    /// The site and page adapters for a slug outside the allowed characters.
    const SLUG_INVALID = "A slug can only contain letters, numbers, underscores, and hyphens.";
    /// The site and page adapters for a slug in use.
    fn slug_taken(slug) = "The slug \"{slug}\" is already taken.";
    /// `SitePageAdapter` for a page without a site.
    const PAGE_SITE_REQUIRED = "A page must belong to a site.";
    /// `SitePageAdapter` for an empty title.
    const PAGE_TITLE_REQUIRED = "A page must have a title.";
    /// `SitePageAdapter` for a block without a layout.
    const BLOCK_LAYOUT_REQUIRED = "All blocks must have a layout.";
    /// `SitePageAdapter` for scalar block data.
    const BLOCK_DATA_SCALAR = "Block data must not be a scalar value.";
    /// `o:site_settings` that is neither an object nor null; `kind` names the
    /// JSON type.
    fn site_settings_not_object(kind) = "Expected an object or null, got {kind}.";
    // The custom CSS sanitiser's refusals (`omeka_site::theme::custom_css`),
    // one per kind. The Theme page marks the line it reads from the English.

    /// More than the sanitiser's byte limit.
    fn css_too_large(line, column, max, bytes) =
        "Custom CSS: Line {line}, column {column}: custom CSS may be at most {max} bytes; this is {bytes}.";
    /// An `@import` rule.
    fn css_import(line, column) = "Custom CSS: Line {line}, column {column}: @import is not allowed.";
    /// An `@document` rule.
    fn css_document(line, column) =
        "Custom CSS: Line {line}, column {column}: @document is not allowed.";
    /// A URL outside the site's `/files/` and `/assets/`.
    fn css_foreign_url(line, column, url) =
        "Custom CSS: Line {line}, column {column}: url({url}) is not allowed; a URL must be a path under /files/ or /assets/.";
    /// `expression()`.
    fn css_expression(line, column) =
        "Custom CSS: Line {line}, column {column}: expression() is not allowed.";
    /// The `behavior` property.
    fn css_behavior(line, column) =
        "Custom CSS: Line {line}, column {column}: behavior is not allowed.";
    /// The `-moz-binding` property.
    fn css_moz_binding(line, column) =
        "Custom CSS: Line {line}, column {column}: -moz-binding is not allowed.";
    /// `</style`, which would end the element.
    fn css_style_breakout(line, column) =
        "Custom CSS: Line {line}, column {column}: </style is not allowed.";
    /// A `url(` the tokenizer cannot read.
    fn css_bad_url(line, column) =
        "Custom CSS: Line {line}, column {column}: this url( is not valid CSS.";
    /// A string that a newline or the end of the text cuts off.
    fn css_unclosed_string(line, column) =
        "Custom CSS: Line {line}, column {column}: this string is not closed.";
    /// A `)`, `]` or `}` with no opening bracket.
    fn css_stray_close(line, column, bracket) =
        "Custom CSS: Line {line}, column {column}: {bracket} closes nothing.";
    /// `SiteAdapter::validateNavigation`.
    const NAV_NOT_ARRAY = "Invalid navigation: navigation must be an array";
    /// `SiteAdapter::validateNavigation`.
    const NAV_LINK_MISSING_TYPE = "Invalid navigation: link missing type";
    /// `SiteAdapter::validateNavigation`.
    const NAV_LINK_MISSING_DATA = "Invalid navigation: link missing data";
    /// `SiteAdapter::validateNavigation`.
    const NAV_INVALID_LINK_DATA = "Invalid navigation: invalid link data";
    /// `SiteAdapter::validateNavigation`.
    const NAV_PAGE_LINKS_NOT_UNIQUE = "Invalid navigation: page links must be unique";
    /// `SiteAdapter::validateNavigation`.
    const NAV_LINKS_NOT_ARRAY = "Invalid navigation: links must be an array";
    /// `Site\Navigation\Link\Page::isValid`.
    const NAV_PAGE_LINK_MISSING_PAGE = "Invalid navigation: page link missing page ID";
    /// `Site\Navigation\Link\Url::isValid`.
    const NAV_URL_MISSING_LABEL = "Invalid navigation: URL link missing label";
    /// `Site\Navigation\Link\Url::isValid`.
    const NAV_URL_MISSING_URL = "Invalid navigation: URL link missing URL";
    /// `Site\Navigation\Link\Url::isValid`.
    const NAV_URL_INVALID_CHARACTER = "Invalid navigation: URL link contains invalid character";
    /// `Site\Navigation\Link\Url::isValid`.
    const NAV_URL_INVALID_SCHEME = "Invalid navigation: URL link invalid scheme";

    // The user and global settings (`crate::settings`).

    /// `AclFactory` grants the settings controller to `global_admin` only.
    const SETTINGS_DENIED = "Permission denied for the current user to access the global settings.";
    /// An `o:user_settings` or global settings body that is not a JSON object.
    const SETTINGS_NOT_OBJECT = "Expected an object.";
    /// A settings key the admin has no field for.
    fn setting_unavailable(key) = "The setting {key} is not available.";
    /// A settings value that is neither a string nor null.
    fn setting_not_text(key) = "The setting {key} must be a string or null.";
    /// A switch setting whose value is not a boolean.
    fn setting_not_flag(key) = "The setting {key} must be true, false or null.";
    /// A list setting whose value is not a list of strings.
    fn setting_not_names(key) = "The setting {key} must be a list of strings or null.";
    /// A count setting written as anything but a whole number above zero.
    fn setting_not_count(key) = "The setting {key} must be a whole number above zero, or null.";

    // Publishing term lists and vocabularies (`crate::publish`). Romeka's own rule.

    /// A write that drops or renames a published term.
    fn publication_terms_removed(terms) =
        "A published term cannot be removed or renamed: {terms}. Deprecate it instead.";
    /// A write that moves a published vocabulary's namespace.
    const PUBLICATION_NAMESPACE_FIXED =
        "A published vocabulary keeps its namespace, so its terms keep their URIs.";
    /// A delete of a published term list, vocabulary or term.
    const PUBLICATION_NOT_DELETABLE =
        "A published term list or vocabulary cannot be deleted. Deprecate its terms instead.";
    /// An undeprecate of a deprecation a version already carries.
    fn publication_deprecation_fixed(term) =
        "The deprecation of \"{term}\" is published and cannot be taken back.";
    /// A publish of a list of items.
    const PUBLICATION_ITEMS_LIST =
        "A list of items cannot be published. Each item already has its own address.";
    /// A publish of a vocabulary whose namespace is someone else's.
    fn publication_foreign_namespace(namespace) =
        "The namespace {namespace} is not this archive's. Publish a vocabulary whose namespace is under this archive's /vocab/ address, or at w3id.org or purl.org.";
    /// A slug that is not lowercase letters, digits and single hyphens.
    const PUBLICATION_SLUG_INVALID =
        "An address name uses lowercase letters, digits and single hyphens, up to 64 characters.";
    /// A slug another publication holds.
    fn publication_slug_taken(slug) = "The address /vocab/{slug}/ is already taken.";
    /// A chosen namespace that is not at a persistent address service.
    const PUBLICATION_NAMESPACE_NOT_PERSISTENT =
        "A namespace other than this archive's must be at w3id.org or purl.org and end with / or #.";
    /// A publish that names another slug or namespace than the first publish fixed.
    const PUBLICATION_ADDRESS_FIXED =
        "A published term list or vocabulary keeps the address it was first published at.";
    /// A deprecate of a term no version carries yet.
    fn publication_term_not_published(term) =
        "\"{term}\" is not published yet, so it cannot be deprecated. Remove it instead.";
    /// A command that names a term the scheme does not have.
    fn publication_unknown_term(term) = "\"{term}\" is not a term of this list or vocabulary.";
    /// A match that is not an absolute http or https URI.
    const PUBLICATION_MATCH_INVALID = "A match must be an absolute http or https URI.";
    /// A match on a vocabulary's term.
    const PUBLICATION_MATCH_TERM_LISTS = "Only the terms of a term list link to an authority.";
    /// A term whose namespace and local name do not make a valid IRI.
    fn publication_term_iri_invalid(uri) = "The term URI {uri} is not a valid IRI.";

    // Import v2 (`docs/import-v2.md`, "Wire"). Romeka's own routes: upstream
    // has no msgid for these, so the text is plain English for archivists.

    /// Any import route, for a caller who is neither a global nor a site admin.
    const IMPORT_ADMINS_ONLY = "Only a global or site administrator may import an archive.";
    /// An import id that names no import.
    fn import_not_found(id) = "No import has the id {id}.";
    /// A request body the import routes cannot read.
    fn import_request_unreadable(reason) = "The request cannot be read: {reason}";
    /// A plan the boundary refuses; the reason is the parser's English.
    fn import_plan_refused(reason) = "The import's settings cannot be saved: {reason}";
    /// 409 `stale`: another save landed after the caller read the plan.
    const IMPORT_STALE = "Someone else saved this import after you opened it. Reload it to see their changes.";
    /// 409 on a plan save once the import has started.
    const IMPORT_PLAN_FROZEN = "This import has already started, so its settings cannot change. Make a copy to change it.";
    /// 409 `v1Plan`: the plan was made by the first importer.
    const IMPORT_V1_PLAN = "This import was made by the old importer, so it cannot be opened or copied here.";
    /// 409 `typeExists` on a plan save.
    fn import_type_exists(label) = "An item type named \"{label}\" is already in Omeka. Choose another name for the new type.";
    /// 409 on deleting an import that is not a draft.
    const IMPORT_DELETE_NOT_DRAFT = "Only an import that is not added yet can be deleted. An import that has run stays in Past imports.";
    /// 409 `changed`: the run derived another model than the one checked.
    const IMPORT_CHANGED = "The files or the import's settings changed after the last check. Check the import again before you run it.";
    /// 409 `blocked`: the check lists problems that stop the run.
    const IMPORT_BLOCKED = "This import cannot run until the problems the check lists are fixed.";
    /// 409 `nothing`: no record would be written.
    const IMPORT_NOTHING_TO_RUN = "This import has nothing to write.";
    /// 409 `confirm`: a delete run needs the number it deletes.
    fn import_confirm_delete(count) = "This import deletes {count} items, media and item sets and cannot be undone. Confirm with that number to run it.";
    /// 409 `busy`: one import at a time, install-wide.
    const IMPORT_BUSY = "Another import is running or being undone. Try again when it has ended.";
    /// 409 on run for a status that cannot run.
    const IMPORT_CANNOT_RUN = "Only an import that is not added yet, that stopped before the end, or that finished with failures can run.";
    /// 409 on undo for a status that cannot be undone.
    const IMPORT_CANNOT_UNDO = "Only an import that has finished, or that stopped before the end, can be undone.";
    /// 409 on stop for an import that is not running.
    const IMPORT_NOT_RUNNING = "This import is not running.";
    /// A source names an archive folder the server does not have.
    fn import_no_root(root) = "There is no archive folder named \"{root}\".";
    /// A folder path that resolves outside its archive folder.
    fn import_outside_root(path) = "The folder \"{path}\" is outside the archive folder.";
    /// A folder path with nothing at it.
    fn import_folder_missing(path) = "There is no folder \"{path}\" in the archive.";
    /// A folder path that names a file.
    fn import_not_a_folder(path) = "\"{path}\" is a file, not a folder.";
    /// 409 on a rescan while the import's job works on it.
    const IMPORT_SCAN_BUSY = "This import is running or being undone. Look at the files again once that has ended.";
    /// 404 for `left.csv` before a run has ended.
    const IMPORT_NO_LEFT_FILE = "This import has no list of left-out files yet. It is written when a run ends.";
    /// The job runner refused to start the job.
    fn import_job_not_started(reason) = "The import could not start: {reason}";
    /// `template.csv?columns=` names a column that is no property, item set, item type or visibility.
    fn import_list_column_unknown(column) = "The list cannot have the column \"{column}\". Choose a property, the item sets, the item type or the visibility.";
    /// `POST …/sheets` without exactly one file in the form.
    const IMPORT_LIST_ONE_FILE = "Send one list file in the form.";
    /// `POST …/sheets` with a file that is not a CSV, TSV or workbook.
    fn import_list_type(name) = "\"{name}\" is not a .csv, .tsv, .xlsx, .xls or .ods file. Save the list as one of these, then send it again.";
    /// `POST …/sheets` with a file over 50 MB.
    const IMPORT_LIST_TOO_LARGE = "The list is larger than 50 MB.";
    /// `POST …/sheets` with a list the sheet reader refuses.
    fn import_list_unread(reason) = "The list cannot be read: {reason}";
    /// `template.csv` or `POST …/sheets` with no `?series=` on a plan of several series.
    const IMPORT_LIST_SERIES_NEEDED = "This plan has several series. Choose the series the list is for.";
    /// `template.csv` or `POST …/sheets` with a `?series=` the plan does not have.
    fn import_list_series_unknown(series) = "The plan has no series \"{series}\".";
    /// `POST …/sheets` with a `?replace=` that names no uploaded list of the series.
    fn import_list_unknown(list) = "The plan has no list \"{list}\".";
    /// `POST …/sheets` with a list whose header has no Identifier column.
    const IMPORT_LIST_NO_IDENTIFIER = "The list has no Identifier column, so its rows cannot find their items. Keep the Identifier column of the list you downloaded.";
    /// `POST …/pairs` with a list that lacks the Row column or the Files column.
    const IMPORT_PAIRS_COLUMNS = "The list needs the Row column and the Files column. Keep both columns of the list you downloaded.";
    /// A run failure: a file the record names could not be opened or read.
    fn import_file_unreadable(path, reason) = "The file \"{path}\" could not be read: {reason}";
    /// A run failure: the install's file settings refused a file.
    fn import_file_refused(path, reason) = "The file \"{path}\" was not added: {reason}";
    /// A run failure: the file store did not take a file's bytes.
    fn import_file_not_stored(path, reason) = "The file \"{path}\" could not be stored: {reason}";
    /// A run failure: an address the media ingester refused or could not fetch.
    fn import_source_refused(source, reason) = "\"{source}\" was not added: {reason}";
    /// A run failure: a media record with neither a file nor an address.
    const IMPORT_MEDIA_NO_SOURCE = "This media names no file and no address to add.";
    /// A run failure: a media record whose item was not added.
    const IMPORT_MEDIA_NO_ITEM = "This media's item was not added, so the media was not added either.";
    /// A run failure: the resource a record changes was deleted since the check.
    const IMPORT_RESOURCE_GONE = "The item, media or item set this record changes is no longer in Omeka.";
    /// A run failure: the database refused the record's rows.
    fn import_record_not_saved(reason) = "This record could not be saved: {reason}";
    /// A run failure: the second pass could not write a record's links.
    fn import_links_not_saved(reason) = "The links of this record could not be saved: {reason}";
    /// A run failure: the API refused to delete what a `delete` record matched.
    fn import_record_not_deleted(reason) = "This was not deleted: {reason}";
    /// 409 `cannotUndo` for an import that deleted resources.
    const IMPORT_DELETE_NOT_UNDONE = "An import that deleted items, media or item sets cannot be undone.";
    /// A run failure: an added item did not join the sites that take new items.
    fn import_sites_not_joined(reason) = "This item was added, but it did not join its sites: {reason}";
    /// Undo kept a resource: its `modified` is later than the import's write.
    const IMPORT_UNDO_EDITED = "This was edited after the import wrote it.";
    /// Undo kept a resource: something outside the import links to it.
    const IMPORT_UNDO_LINKED = "An item, media or item set outside this import links to this.";
    /// Undo kept an item: media were added to it outside the import.
    const IMPORT_UNDO_MEDIA_ADDED = "Media were added to this item outside this import.";
    /// Undo kept an item set: items outside the import were put in it.
    const IMPORT_UNDO_MEMBERS_ADDED = "Items outside this import were put in this item set.";
    /// Undo kept a resource: a later import changed it.
    fn import_undo_later_import(import) = "Import {import} changed this after this import did.";
    /// Undo kept an item type or a field: something outside the import uses it.
    const IMPORT_UNDO_IN_USE = "Items, media, item sets or item types outside this import use this.";
    /// Undo kept a media because undo keeps its item.
    const IMPORT_UNDO_ITEM_KEPT = "The item of this media is kept, so the media is kept too.";
    /// Undo kept an item set because undo keeps an item in it.
    const IMPORT_UNDO_MEMBER_KEPT = "An item in this item set is kept, so the item set is kept too.";
    /// Undo stopped on an error; a new undo goes on from there.
    fn import_undo_stopped(reason) = "The undo stopped before the end: {reason}";
    /// An upload by someone who neither started the import nor is a global admin.
    const IMPORT_UPLOAD_OWNER_ONLY = "Only the person who started this import, or a global administrator, can add files to it.";
    /// An upload into an import that has run.
    const IMPORT_UPLOAD_NOT_DRAFT = "Files can be added only before an import runs. To add more files, make a copy of this import.";
    /// An upload whose file name the path rules refuse.
    fn import_upload_path(path, reason) = "The file name \"{path}\" cannot be used: {reason}.";
    /// An upload larger than `import_upload_max_file`.
    fn import_upload_too_large(limit) = "This file is larger than {limit} bytes, the largest file an import takes.";
    /// An upload that would take the import past `import_upload_max_total`.
    fn import_upload_full(limit) = "The files of this import would pass {limit} bytes, the most one import holds. Remove some files, or let the files still on their way finish, and try again.";
    /// One `PATCH` with more bytes than the chunk limit.
    fn import_upload_chunk_too_large(limit) = "This request carries more than {limit} bytes. Send the file in smaller chunks.";
    /// A new upload for a path an unfinished upload holds.
    fn import_upload_in_progress(path) = "\"{path}\" is still on its way. Go on with that upload instead of starting it again.";
    /// A tus request naming an upload the import does not hold.
    fn import_upload_not_found(upload) = "This import has no upload with the id \"{upload}\".";
    /// A `PATCH` whose `Upload-Offset` is not the stored offset.
    fn import_upload_offset(received) = "The server has {received} bytes of this file. Send the bytes that come after them.";
    /// A `PATCH` that would pass the declared `Upload-Length`.
    const IMPORT_UPLOAD_PAST_LENGTH = "These bytes go past the size this upload declared.";
    /// A `POST` without a usable `Upload-Length`.
    const IMPORT_UPLOAD_LENGTH = "Send Upload-Length with the size of the file in bytes.";
    /// A `POST` without a file name in `Upload-Metadata`.
    const IMPORT_UPLOAD_METADATA = "Send Upload-Metadata with the file name, as \"filename\" and its name in base64.";
    /// A `PATCH` without a usable `Upload-Offset`.
    const IMPORT_UPLOAD_OFFSET_MISSING = "Send Upload-Offset with the number of bytes already sent.";
    /// A `PATCH` whose body is not `application/offset+octet-stream`.
    const IMPORT_UPLOAD_CONTENT_TYPE = "Send the bytes with Content-Type application/offset+octet-stream.";
    /// A tus request without `Tus-Resumable: 1.0.0`.
    const IMPORT_UPLOAD_VERSION = "This server speaks version 1.0.0 of the resumable upload protocol. Send Tus-Resumable: 1.0.0.";
    /// A `PATCH` whose body stopped before the request ended; nothing of it was kept.
    const IMPORT_UPLOAD_INTERRUPTED = "The bytes stopped arriving before the request ended, so none of them were kept. Ask for the offset and send them again.";
    /// A ZIP entry whose path would leave its folder, or is absolute.
    fn import_zip_escapes(name) = "\"{name}\" in the ZIP would land outside its folder, so it was left out.";
    /// A ZIP entry that is a symbolic link.
    fn import_zip_link(name) = "\"{name}\" in the ZIP is a link, so it was left out.";
    /// A ZIP entry whose name the path rules refuse.
    fn import_zip_unusable(name, reason) = "\"{name}\" in the ZIP has a name that cannot be used ({reason}), so it was left out.";
    /// A ZIP entry that is encrypted or packed with a method the reader lacks.
    fn import_zip_unsupported(name) = "\"{name}\" in the ZIP is encrypted or packed in a way Romeka S cannot unpack, so it was left out.";
    /// A ZIP entry renamed because an earlier entry has its name when case is ignored.
    fn import_zip_renamed(name, renamed) = "\"{name}\" in the ZIP has the name of an earlier file when capitals are ignored, so it was added as \"{renamed}\".";
    /// A ZIP refused: an entry unpacks to more than 100 times its packed size and 10 MB.
    fn import_zip_bomb(name) = "The ZIP was refused: \"{name}\" unpacks to more than 100 times its packed size. Nothing from it was added.";
    /// A ZIP refused: its files would pass `import_upload_max_total`.
    fn import_zip_full(limit) = "The ZIP was refused: its files would pass {limit} bytes, the most one import holds. Nothing from it was added.";
    /// A ZIP refused: more than 100,000 entries.
    fn import_zip_too_many(count) = "The ZIP was refused: it holds {count} files, more than 100,000. Nothing from it was added.";
    /// A ZIP refused: an entry larger than `import_upload_max_file`.
    fn import_zip_file_too_large(name, limit) = "The ZIP was refused: \"{name}\" is larger than {limit} bytes, the largest file an import takes. Nothing from it was added.";
    /// A ZIP refused: its directory or an entry's bytes do not read.
    fn import_zip_unreadable(reason) = "The ZIP was refused: it could not be read ({reason}). Nothing from it was added.";
    /// A ZIP unpacked.
    fn import_zip_unpacked(count) = "The ZIP was unpacked: {count} files were added.";
    /// The staging sweep ended.
    fn import_sweep_done(imports, uploads) = "The files of {imports} imports were removed, and {uploads} unfinished uploads.";
    /// A run's media whose playable-copy decision failed; the import stands.
    fn import_access_failed(media, reason) = "No playable copy was queued for media {media}: {reason}";
    /// A run whose ledger could not be read for its playable copies.
    fn import_access_unread(reason) = "No playable copies were queued for this import: {reason}";
    /// A staged upload named by a media create that the caller's account does not hold.
    fn media_upload_not_found(upload) = "This account has no staged upload with the id \"{upload}\".";
    /// A media create naming a staged upload another create is adding right now.
    fn media_upload_busy(upload) = "This upload \"{upload}\" is being added right now. Wait for that to finish and try again.";
    /// A media create naming a staged upload whose bytes have not all arrived.
    fn media_upload_incomplete(received, length) = "This upload holds {received} of its {length} bytes. Send the rest before you add it.";
    /// A media create naming a staged upload with an ingester other than upload.
    const MEDIA_UPLOAD_INGESTER = "A staged upload is added with the upload ingester.";
    /// A retry on a media whose playable copy has nothing to make again.
    const NO_DERIVATIVE_TO_RETRY = "This media has no failed copy to make again.";
    /// An inline `o:media` entry naming a staged upload.
    const MEDIA_UPLOAD_INLINE = "A staged upload is added with its own media create, not inside an item.";
    /// A staged upload larger than `import_upload_max_file`.
    fn upload_too_large(limit) = "This file is larger than {limit} bytes, the largest file a media upload takes.";
    /// A link that is not an absolute http(s) address.
    fn media_link_invalid(url) = "The link \"{url}\" is not an http or https address.";
    /// A link the guarded client could not read.
    fn media_link_unreadable(url, reason) = "The link \"{url}\" could not be read: {reason}";
    /// A link that answered with a status other than 2xx.
    fn media_link_status(url, status, reason) = "The link \"{url}\" answered {status} {reason}.";
    /// An upload whose `item` metadata is not an existing item's id.
    const UPLOAD_ITEM = "An upload's item id must name an item that exists.";
    /// The sweep removed staged uploads whose 24 hours had passed.
    fn media_upload_sweep_done(count) = "{count} staged uploads passed their kept time and were removed.";

    // Bulk changes to items (`briefs/items-bulk-edit-design.md`). Romeka's own routes: upstream
    // has no msgid for these.

    const BULK_CANNOT_RUN = "Only a previewed bulk change, or one that stopped or failed before its end, can run.";
    const BULK_EXPIRED = "This preview is older than ten minutes. Preview the change again.";
    const BULK_CANNOT_UNDO = "Only a bulk change that has ended and is not undone yet can be undone.";
    const BULK_NOT_RUNNING = "This bulk change is not running.";
    const BULK_ITEM_GONE = "This item is no longer in Omeka.";
    const BULK_ITEM_DENIED = "The person who started this change may not edit this item.";
    const BULK_ITEM_TEMPLATE_REQUIRES = "The new item type needs values this item does not have.";
    const BULK_ITEM_REMOVES_REQUIRED = "The item type needs a value in a field this change would empty, so the change left the item as it is.";
    const BULK_ITEM_UNDO_DENIED = "The person who asked for the undo may not edit this item, so the undo left it as it is.";
    const BULK_ITEM_DELETE_DENIED = "The person who started this change may not delete this item.";
    const BULK_ITEM_ID_TAKEN = "Another record now has this item's id, so the undo left the item in the trash.";
    const BULK_ITEM_LINKS_LEFT = "The item is back, but links to it from items edited since the delete were left out.";
    const BULK_PURGED = "The trash of this change was emptied, so its items cannot come back.";
}

/// The registry as `serverMessages.json` holds it: catalogue key to pattern,
/// in registry order.
#[must_use]
pub fn catalogue() -> serde_json::Value {
    let messages: serde_json::Map<String, serde_json::Value> = REGISTRY
        .iter()
        .map(|message| (message.key(), message.pattern.into()))
        .collect();
    serde_json::json!({ "messages": messages })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::collections::BTreeSet;

    use super::*;

    /// Where the admin reads the generated registry.
    const ADMIN_FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin/src/generated/serverMessages.json"
    );

    /// The admin's file is the registry's serialisation, byte for byte. A
    /// difference fails with the command that rewrites it.
    #[test]
    fn generated_registry_matches_the_admin_file() {
        let rendered = format!(
            "{}\n",
            serde_json::to_string_pretty(&catalogue()).expect("the registry serialises")
        );
        if std::env::var_os("UPDATE_GENERATED").is_some() {
            std::fs::write(ADMIN_FILE, &rendered).expect("the registry file is writable");
            return;
        }
        let current = std::fs::read_to_string(ADMIN_FILE).unwrap_or_default();
        assert_eq!(
            current, rendered,
            "run UPDATE_GENERATED=1 cargo test -p omeka-api messages to rewrite {ADMIN_FILE}"
        );
    }

    /// Two names folding to one key, or two names sharing one pattern, would
    /// make the admin's lookup ambiguous.
    #[test]
    fn keys_and_patterns_are_unique() {
        let keys: BTreeSet<String> = REGISTRY.iter().map(ServerMessage::key).collect();
        let patterns: BTreeSet<&str> = REGISTRY.iter().map(|message| message.pattern).collect();
        assert_eq!(keys.len(), REGISTRY.len());
        assert_eq!(patterns.len(), REGISTRY.len());
    }

    /// A hole is `{name}` with a plain lowercase name, the shape the admin's
    /// interpolation reads, and no pattern escapes a brace.
    #[test]
    fn holes_are_plain_names() {
        for message in REGISTRY {
            assert!(!message.pattern.contains("{{"), "{}", message.name);
            for hole in message.pattern.split('{').skip(1) {
                let name = hole.split('}').next().unwrap_or_default();
                assert!(
                    !name.is_empty() && name.chars().all(|ch| ch.is_ascii_lowercase()),
                    "{}: {{{name}}}",
                    message.name
                );
            }
        }
    }

    /// A function fills its holes with the values, in the pattern's order.
    #[test]
    fn a_pattern_fills_its_holes() {
        assert_eq!(slug_taken("home"), "The slug \"home\" is already taken.");
        assert_eq!(
            entity_not_found("Omeka\\Entity\\Item", "{\"id\":\"9\"}"),
            "Omeka\\Entity\\Item entity with criteria {\"id\":\"9\"} not found"
        );
        assert_eq!(
            REGISTRY[0],
            ServerMessage {
                name: "METHOD_NOT_ALLOWED",
                pattern: "Method Not Allowed"
            }
        );
        assert_eq!(REGISTRY[0].key(), "server.methodNotAllowed");
    }
}
