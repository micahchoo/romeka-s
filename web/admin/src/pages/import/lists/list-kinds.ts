/**
 * The kinds of file a list may be: the extensions the server's list reader
 * takes (`crates/omeka-server/src/imports/lists.rs`, `read_upload`). The
 * chooser offers these and its hint names them.
 */
export const LIST_EXTENSIONS = ['.csv', '.tsv', '.xlsx', '.xls', '.ods'] as const;

/** The `accept` of a list chooser. */
export const LIST_ACCEPT = LIST_EXTENSIONS.join(',');

