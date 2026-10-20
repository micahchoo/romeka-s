/**
 * `#/resource-templates/new`: "What kind of thing is this?" first, then the
 * editor with the kind's class and starter fields, or a title alone.
 */
import { useId, useState } from "react";
import type { ReactElement } from "react";
import type { Starter, StarterTable } from "../../api/starters";
import { plainName } from "../../app/plain-terms";
import { useLocale } from "../../i18n";
import { useTermLabels } from "../../i18n/termText";
import { TypeEditor } from "./editor";
import { useItemTypesText } from "./messages";
import type { TypeDraft } from "./model";
import type { TypeOptions } from "./options";
import { draftFromStarter, starterFor } from "./starter-draft";
import { starterLabel } from "./starter-text";

const titleOnly = (starter: Starter): Starter => ({
  ...starter,
  fields: starter.fields.slice(0, 1),
});

export const NewType = ({
  options,
  table,
}: {
  readonly options: TypeOptions;
  readonly table: StarterTable;
}): ReactElement => {
  const t = useItemTypesText();
  const locale = useLocale();
  const text = useTermLabels();
  const inputId = useId();
  const [kind, setKind] = useState("");
  const [draft, setDraft] = useState<TypeDraft | null>(null);

  if (draft !== null) {
    return (
      <TypeEditor
        template={null}
        initial={draft}
        options={options}
        onSaved={(created) => {
          window.location.hash = `#/resource-templates/${String(created.id)}`;
        }}
      />
    );
  }

  const name = kind.trim();
  const known = starterFor(table.starters, name);
  const starter = known ?? table.other;
  const start = (chosen: Starter): void => {
    setDraft(
      draftFromStarter(
        name,
        chosen,
        locale,
        options.properties,
        options.classes,
      ),
    );
  };
  const fields = starter.fields.flatMap((field) => {
    const property = options.properties.find(
      (entry) => entry.term === field.term,
    );
    return property === undefined ? [] : [{ field, property }];
  });
  const className = options.classes.find(
    (entry) => entry.term === starter.classTerm,
  );

  return (
    <section className="type-ask">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (name !== "") {
            start(starter);
          }
        }}
      >
        <p className="type-ask-question">
          <label htmlFor={inputId}>{t("ask.question")}</label>
          <input
            id={inputId}
            type="text"
            // The page exists to ask this one question.
            autoFocus
            value={kind}
            placeholder={t("ask.placeholder")}
            onChange={(event) => {
              setKind(event.target.value);
            }}
          />
        </p>
        <p className="field-note">{t("ask.hint")}</p>
      </form>
      {name === "" ? null : (
        <div className="type-ask-card">
          <h2>
            {known === null
              ? t("ask.unknown", { name })
              : t("ask.known", { name })}
          </h2>
          {className === undefined ? null : (
            <p className="type-class-line">
              {t("class.line", { name: plainName(className, text) })}{" "}
              <code>{className.term}</code>
            </p>
          )}
          <ul className="type-ask-fields">
            {fields.map(({ field, property }) => (
              <li key={field.term}>
                {starterLabel(field, locale)} <code>{property.term}</code>
              </li>
            ))}
          </ul>
          <p className="type-ask-actions">
            <button
              type="button"
              className="button-primary"
              onClick={() => {
                start(starter);
              }}
            >
              {known === null ? t("ask.start") : t("ask.useFields")}
            </button>
            {known === null ? null : (
              <button
                type="button"
                onClick={() => {
                  start(titleOnly(starter));
                }}
              >
                {t("ask.titleOnly", {
                  field:
                    starter.fields[0] === undefined
                      ? ""
                      : starterLabel(starter.fields[0], locale),
                })}
              </button>
            )}
          </p>
          <p className="field-note">{t("ask.after")}</p>
        </div>
      )}
    </section>
  );
};
