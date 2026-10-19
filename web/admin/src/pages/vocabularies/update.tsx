/**
 * "Update from a file or web address" on a vocabulary's page:
 * `Admin/VocabularyController::editAction` with a file, then `::updateAction`.
 *
 * The panel moves through three states: choosing the source, reviewing the
 * diff the server computed, and done. Nothing is written until the reviewer
 * applies the diff they were shown, and the server refuses it if a member
 * changed in between. Members the file no longer has are listed as kept,
 * because upstream never deletes them and the values that use them stay.
 */
import type { ReactElement, SyntheticEvent } from "react";
import { useState } from "react";
import { api } from "../../api/client";
import type {
  Vocabulary,
  VocabularyDiff,
  VocabularyMemberDiff,
} from "../../api/types";
import { FormErrors } from "../../app/errors";
import { EditPanel } from "../../app/fields";
import { toError } from "../../app/useAsync";
import { useTranslator } from "../../i18n";
import type { ImportText } from "./messages";
import { useImportText } from "./messages";
import {
  AdvancedFields,
  EMPTY_SOURCE,
  SourceFields,
  errorLines,
  readSource,
} from "./source";
import type { SourceDraft } from "./source";

type Step =
  | { readonly kind: "source" }
  | { readonly kind: "review"; readonly diff: VocabularyDiff }
  | { readonly kind: "done" };

const writes = (diff: VocabularyMemberDiff): number =>
  diff.added.length + diff.label.length + diff.comment.length;

const shown = (value: string | null, text: ImportText): string =>
  value === null || value === "" ? text("diff.blank") : value;

const NewTable = ({
  heading,
  prefix,
  members,
}: {
  readonly heading: string;
  readonly prefix: string;
  readonly members: VocabularyMemberDiff["added"];
}): ReactElement | null => {
  const text = useImportText();
  if (members.length === 0) {
    return null;
  }
  return (
    <>
      <h4>{heading}</h4>
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              <th>{text("diff.term")}</th>
              <th>{text("diff.label")}</th>
              <th>{text("diff.comment")}</th>
            </tr>
          </thead>
          <tbody>
            {members.map((member) => (
              <tr key={member.localName}>
                <td>{`${prefix}:${member.localName}`}</td>
                <td>{member.label}</td>
                <td>{shown(member.comment, text)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
};

/** Every changed label and comment, classes first, in one table. */
const ChangedTable = ({
  prefix,
  diff,
}: {
  readonly prefix: string;
  readonly diff: VocabularyDiff;
}): ReactElement | null => {
  const text = useImportText();
  const rows = [diff.classes, diff.properties].flatMap((kind) => [
    ...kind.label.map((change) => ({ ...change, what: text("diff.label") })),
    ...kind.comment.map((change) => ({
      ...change,
      what: text("diff.comment"),
    })),
  ]);
  if (rows.length === 0) {
    return null;
  }
  return (
    <>
      <h4>{text("diff.changed", { count: rows.length })}</h4>
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              <th>{text("diff.term")}</th>
              <th>{text("diff.what")}</th>
              <th>{text("diff.now")}</th>
              <th>{text("diff.file")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={`${row.localName} ${row.what}`}>
                <td>{`${prefix}:${row.localName}`}</td>
                <td>{row.what}</td>
                <td>{shown(row.from, text)}</td>
                <td>{shown(row.to, text)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
};

const KeptList = ({
  prefix,
  diff,
}: {
  readonly prefix: string;
  readonly diff: VocabularyDiff;
}): ReactElement | null => {
  const text = useImportText();
  const kept = [...diff.classes.removed, ...diff.properties.removed];
  if (kept.length === 0) {
    return null;
  }
  return (
    <>
      <h4>{text("diff.removed", { count: kept.length })}</h4>
      <p>{text("diff.removedNote")}</p>
      <ul className="kept-members">
        {kept.map((member) => (
          <li
            key={member.localName}
          >{`${member.label} (${prefix}:${member.localName})`}</li>
        ))}
      </ul>
    </>
  );
};

const Review = ({
  vocabulary,
  diff,
  onApplied,
  onCancel,
}: {
  readonly vocabulary: Vocabulary;
  readonly diff: VocabularyDiff;
  readonly onApplied: () => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const translator = useTranslator();
  const text = useImportText();
  const [errors, setErrors] = useState<readonly string[]>([]);
  const [busy, setBusy] = useState(false);
  const changes = writes(diff.classes) + writes(diff.properties);
  const apply = (): void => {
    setBusy(true);
    api
      .applyVocabularyDiff(vocabulary.id, diff)
      .then(onApplied, (error: unknown) => {
        setErrors(errorLines(toError(error), translator));
        setBusy(false);
      });
  };
  return (
    <section className="vocabulary-diff" aria-label={text("diff.review")}>
      <p role="status">
        {changes === 0 ? text("diff.none") : text("diff.review")}
      </p>
      <NewTable
        heading={text("diff.newClasses", { count: diff.classes.added.length })}
        prefix={vocabulary.prefix}
        members={diff.classes.added}
      />
      <NewTable
        heading={text("diff.newProperties", {
          count: diff.properties.added.length,
        })}
        prefix={vocabulary.prefix}
        members={diff.properties.added}
      />
      <ChangedTable prefix={vocabulary.prefix} diff={diff} />
      <KeptList prefix={vocabulary.prefix} diff={diff} />
      <FormErrors errors={errors} />
      <p className="form-actions">
        {changes === 0 ? null : (
          <button type="button" disabled={busy} onClick={apply}>
            {busy ? text("diff.applying") : text("diff.apply")}
          </button>
        )}
        <button
          type="button"
          className="secondary"
          disabled={busy}
          onClick={onCancel}
        >
          {text("diff.cancel")}
        </button>
      </p>
    </section>
  );
};

const UpdateFlow = ({
  vocabulary,
  onApplied,
}: {
  readonly vocabulary: Vocabulary;
  readonly onApplied: () => void;
}): ReactElement => {
  const translator = useTranslator();
  const text = useImportText();
  const [source, setSource] = useState<SourceDraft>(EMPTY_SOURCE);
  const [step, setStep] = useState<Step>({ kind: "source" });
  const [errors, setErrors] = useState<readonly string[]>([]);
  const [busy, setBusy] = useState(false);

  if (step.kind === "review") {
    return (
      <Review
        vocabulary={vocabulary}
        diff={step.diff}
        onApplied={() => {
          setStep({ kind: "done" });
          onApplied();
        }}
        onCancel={() => {
          setStep({ kind: "source" });
        }}
      />
    );
  }

  const compare = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void (async () => {
      const read = await readSource(source, text);
      if (typeof read === "string") {
        setErrors([read]);
        return;
      }
      setErrors([]);
      setBusy(true);
      try {
        setStep({
          kind: "review",
          diff: await api.diffVocabulary(vocabulary.id, read),
        });
      } catch (error: unknown) {
        setErrors(errorLines(toError(error), translator));
      }
      setBusy(false);
    })();
  };

  return (
    <form onSubmit={compare} noValidate>
      {step.kind === "done" ? (
        <p role="status">{text("diff.applied")}</p>
      ) : null}
      <p>{text("update.lede")}</p>
      <SourceFields idPrefix="update" draft={source} onChange={setSource} />
      <AdvancedFields idPrefix="update" draft={source} onChange={setSource} />
      <FormErrors errors={errors} />
      <p className="form-actions">
        <button type="submit" disabled={busy}>
          {busy ? text("update.comparing") : text("update.compare")}
        </button>
      </p>
    </form>
  );
};

const UpdatePanel = ({
  vocabulary,
  onApplied,
}: {
  readonly vocabulary: Vocabulary;
  readonly onApplied: () => void;
}): ReactElement => {
  const text = useImportText();
  return (
    <EditPanel label={text("update.toggle")}>
      <UpdateFlow vocabulary={vocabulary} onApplied={onApplied} />
    </EditPanel>
  );
};

/** The update panel, with its messages loaded. */
export const VocabularyUpdate = ({
  vocabulary,
  onApplied,
}: {
  readonly vocabulary: Vocabulary;
  readonly onApplied: () => void;
}): ReactElement => <UpdatePanel vocabulary={vocabulary} onApplied={onApplied} />;
