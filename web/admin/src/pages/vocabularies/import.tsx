/**
 * `#/standards/vocabularies/import`: `Admin/VocabularyController::importAction`.
 *
 * The source comes first, because it is the thing the user arrives with; the
 * vocabulary's own namespace, prefix and label follow, then upstream's advanced
 * options under a fold. A successful import opens the new vocabulary's page,
 * which lists every class and property it brought in.
 */
import type { ReactElement, SyntheticEvent } from "react";
import { useState } from "react";
import { api } from "../../api/client";
import { importVocabulary } from "../../api/vocabularies";
import { FormErrors } from "../../app/errors";
import { SubmitRow } from "../../app/fields";
import { toError } from "../../app/useAsync";
import { useTranslator } from "../../i18n";
import { standardsHref } from "../standards/routes";
import { useImportText } from "./messages";
import {
  AdvancedFields,
  EMPTY_SOURCE,
  SourceFields,
  TextInput,
  errorLines,
  readSource,
} from "./source";
import type { SourceDraft } from "./source";
import { common } from "../../i18n/namespaces/common";
import { standards } from "../../i18n/namespaces/standards";

/** `Vocabulary::PREFIX_REGEX`. */
const PREFIX_PATTERN = /^[a-z0-9_-]+$/i;

const ImportForm = (): ReactElement => {
  const translator = useTranslator(common, standards);
  const { t } = translator;
  const text = useImportText();
  const [source, setSource] = useState<SourceDraft>(EMPTY_SOURCE);
  const [namespaceUri, setNamespaceUri] = useState("");
  const [prefix, setPrefix] = useState("");
  const [label, setLabel] = useState("");
  const [comment, setComment] = useState("");
  const [errors, setErrors] = useState<readonly string[]>([]);
  const [busy, setBusy] = useState(false);

  const problems = (): readonly string[] => [
    ...(namespaceUri.trim() === ""
      ? [t("vocabularies.errorEmptyNamespace")]
      : []),
    ...(prefix.trim() === "" ? [t("vocabularies.errorEmptyPrefix")] : []),
    ...(prefix.trim() !== "" && !PREFIX_PATTERN.test(prefix.trim())
      ? [t("vocabularies.errorPrefixPattern")]
      : []),
    ...(label.trim() === "" ? [t("form.errorEmptyLabel")] : []),
  ];

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void (async () => {
      const read = await readSource(source, text);
      const found = [
        ...(typeof read === "string" ? [read] : []),
        ...problems(),
      ];
      setErrors(found);
      if (typeof read === "string" || found.length > 0) {
        return;
      }
      setBusy(true);
      try {
        const created = await importVocabulary(api, 
          {
            namespaceUri: namespaceUri.trim(),
            prefix: prefix.trim(),
            label: label.trim(),
            comment: comment.trim() === "" ? null : comment,
            owner: null,
          },
          read,
        );
        window.location.hash = standardsHref("vocabularies", created.id);
      } catch (error: unknown) {
        setErrors(errorLines(toError(error), translator));
        setBusy(false);
      }
    })();
  };

  return (
    <section className="vocabulary-import">
      <h2>{text("import.heading")}</h2>
      <p className="lede">{text("import.lede")}</p>
      <form onSubmit={submit} noValidate>
        <SourceFields idPrefix="import" draft={source} onChange={setSource} />
        <fieldset>
          <legend>{text("about.legend")}</legend>
          <TextInput
            id="import-namespace-uri"
            type="url"
            label={text("about.namespace")}
            help={text("about.namespaceHelp")}
            value={namespaceUri}
            onChange={setNamespaceUri}
          />
          <TextInput
            id="import-prefix"
            label={text("about.prefix")}
            help={text("about.prefixHelp")}
            value={prefix}
            onChange={setPrefix}
          />
          <TextInput
            id="import-label"
            label={text("about.label")}
            help={text("about.labelHelp")}
            value={label}
            onChange={setLabel}
          />
          <p className="import-field">
            <label htmlFor="import-comment">{text("about.comment")}</label>
            <textarea
              id="import-comment"
              rows={2}
              value={comment}
              onChange={(event) => {
                setComment(event.target.value);
              }}
            />
          </p>
        </fieldset>
        <AdvancedFields idPrefix="import" draft={source} onChange={setSource} />
        <FormErrors errors={errors} />
        {busy ? <p role="status">{text("import.working")}</p> : null}
        <SubmitRow label={text("import.submit")} busy={busy} />
      </form>
    </section>
  );
};

export const VocabularyImportPage = (): ReactElement => <ImportForm />;
