/**
 * Where a vocabulary import or update reads its RDF: a file or a web address,
 * its format, and the advanced label options. The import page and the update
 * panel share the fields, so both offer upstream's `VocabularyForm` choices.
 */
import type { ReactElement } from "react";
import { ApiError } from "../../api/client";
import type { VocabularyFormat, VocabularySource } from "../../api/types";
import { formatError } from "../../app/format";
import type { Translator } from "../../i18n";
import { DefinedTerm } from "../standards/definition";
import type { ImportMessageKey, ImportText } from "./messages";
import { useImportText } from "./messages";
import "./import.css";

export interface SourceDraft {
  readonly kind: "upload" | "url";
  readonly file: File | null;
  readonly url: string;
  readonly format: VocabularyFormat;
  readonly lang: string;
  readonly labelProperty: string;
  readonly commentProperty: string;
}

export const EMPTY_SOURCE: SourceDraft = {
  kind: "upload",
  file: null,
  url: "",
  format: "guess",
  lang: "",
  labelProperty: "",
  commentProperty: "",
};

const FORMATS: readonly (readonly [VocabularyFormat, ImportMessageKey])[] = [
  ["guess", "format.guess"],
  ["rdfxml", "format.rdfxml"],
  ["turtle", "format.turtle"],
  ["ntriples", "format.ntriples"],
  ["jsonld", "format.jsonld"],
];

const isFormat = (value: string): value is VocabularyFormat =>
  FORMATS.some(([format]) => format === value);

/** The extensions the file chooser offers first. Any file can still be chosen. */
const ACCEPT = ".rdf,.owl,.xml,.ttl,.n3,.nt,.jsonld,.json";

/**
 * The source the routes read, or the message that keeps the draft from being
 * sent. An upload is read as text here, so the request carries the file's text.
 */
export const readSource = async (
  draft: SourceDraft,
  text: ImportText,
): Promise<VocabularySource | string> => {
  const options = {
    format: draft.format,
    lang: draft.lang.trim(),
    labelProperty: draft.labelProperty.trim(),
    commentProperty: draft.commentProperty.trim(),
  };
  if (draft.kind === "url") {
    const url = draft.url.trim();
    return url === ""
      ? text("error.noUrl")
      : { ...options, origin: { kind: "url", url } };
  }
  if (draft.file === null) {
    return text("error.noFile");
  }
  return {
    ...options,
    origin: { kind: "upload", text: await draft.file.text() },
  };
};

/** Every message an error carries, one line each: the API's own list when it sent one. */
export const errorLines = (
  error: Error,
  translator: Translator,
): readonly string[] =>
  error instanceof ApiError && error.errors !== null
    ? Object.values(error.errors).flat()
    : [formatError(error, translator)];

/** A field label with a "?" that opens its help in place. */
export const HelpLabel = ({
  htmlFor,
  label,
  help,
}: {
  readonly htmlFor: string;
  readonly label: string;
  readonly help: string;
}): ReactElement => (
  <DefinedTerm term={label} comment={help}>
    <label htmlFor={htmlFor}>{label}</label>
  </DefinedTerm>
);

export const TextInput = ({
  id,
  label,
  help,
  value,
  onChange,
  type = "text",
}: {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly type?: "text" | "url";
}): ReactElement => (
  <p className="import-field">
    <HelpLabel htmlFor={id} label={label} help={help} />
    <input
      id={id}
      type={type}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
      }}
    />
  </p>
);

export const SourceFields = ({
  idPrefix,
  draft,
  onChange,
}: {
  readonly idPrefix: string;
  readonly draft: SourceDraft;
  readonly onChange: (draft: SourceDraft) => void;
}): ReactElement => {
  const text = useImportText();
  const set = (patch: Partial<SourceDraft>): void => {
    onChange({ ...draft, ...patch });
  };
  const id = (name: string): string => `${idPrefix}-${name}`;
  return (
    <fieldset>
      <legend>{text("source.legend")}</legend>
      <p className="choice-row">
        {(["upload", "url"] as const).map((kind) => (
          <label key={kind} htmlFor={id(kind)}>
            <input
              id={id(kind)}
              type="radio"
              name={id("kind")}
              checked={draft.kind === kind}
              onChange={() => {
                set({ kind });
              }}
            />
            {text(kind === "upload" ? "source.upload" : "source.url")}
          </label>
        ))}
      </p>
      {draft.kind === "upload" ? (
        <p className="import-field">
          <label htmlFor={id("file")}>{text("source.file")}</label>
          <input
            id={id("file")}
            type="file"
            accept={ACCEPT}
            onChange={(event) => {
              set({ file: event.target.files?.item(0) ?? null });
            }}
          />
        </p>
      ) : (
        <p className="import-field">
          <label htmlFor={id("url")}>{text("source.urlLabel")}</label>
          <input
            id={id("url")}
            type="url"
            inputMode="url"
            placeholder="https://"
            value={draft.url}
            onChange={(event) => {
              set({ url: event.target.value });
            }}
          />
        </p>
      )}
      <p className="import-field">
        <HelpLabel
          htmlFor={id("format")}
          label={text("source.format")}
          help={text("source.formatHelp")}
        />
        <select
          id={id("format")}
          value={draft.format}
          onChange={(event) => {
            const value = event.target.value;
            if (isFormat(value)) {
              set({ format: value });
            }
          }}
        >
          {FORMATS.map(([format, key]) => (
            <option key={format} value={format}>
              {text(key)}
            </option>
          ))}
        </select>
      </p>
    </fieldset>
  );
};

/** Upstream's advanced fieldset: the preferred language and the label and comment paths. */
export const AdvancedFields = ({
  idPrefix,
  draft,
  onChange,
}: {
  readonly idPrefix: string;
  readonly draft: SourceDraft;
  readonly onChange: (draft: SourceDraft) => void;
}): ReactElement => {
  const text = useImportText();
  const set = (patch: Partial<SourceDraft>): void => {
    onChange({ ...draft, ...patch });
  };
  const id = (name: string): string => `${idPrefix}-${name}`;
  return (
    <details className="import-advanced">
      <summary>{text("vocabularies.advancedSummary")}</summary>
      <TextInput
        id={id("lang")}
        label={text("advanced.lang")}
        help={text("advanced.langHelp")}
        value={draft.lang}
        onChange={(lang) => {
          set({ lang });
        }}
      />
      <TextInput
        id={id("label-property")}
        label={text("advanced.label")}
        help={text("advanced.labelHelp")}
        value={draft.labelProperty}
        onChange={(labelProperty) => {
          set({ labelProperty });
        }}
      />
      <TextInput
        id={id("comment-property")}
        label={text("advanced.comment")}
        help={text("advanced.commentHelp")}
        value={draft.commentProperty}
        onChange={(commentProperty) => {
          set({ commentProperty });
        }}
      />
    </details>
  );
};
