import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Media, MediaWrite } from '../../api/types';
import { toValueWrites, type ResourceDraft } from '../../app/draft';
import { ErrorNotice, FormErrors } from '../../app/errors';
import { ResourceFields, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { loadFormOptions, type FormOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { draftErrors, openDraft, templateById } from '../../app/value-fields';
import { createLocalField } from '../../app/local-fields';
import { ValuesEditor } from '../../app/values-editor';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

interface MediaEditorProps {
  readonly media: Media;
  readonly options: FormOptions;
  readonly onSaved: (media: Media) => void;
}

/**
 * `Admin/MediaController::editAction`: the shared resource columns and values,
 * plus alt text and language. A full `PUT` replaces every value, so the form
 * always sends the values it holds. The thumbnail travels unchanged.
 */
const MediaEditor = ({ media, options, onSaved }: MediaEditorProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const [draft, setDraft] = useState<ResourceDraft>(() =>
    openDraft(media, options.templates, options.properties),
  );
  const [altText, setAltText] = useState(media.altText ?? '');
  const [lang, setLang] = useState(media.lang ?? '');
  const [formErrors, setFormErrors] = useState<readonly string[]>([]);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const errors = draftErrors(draft, options.templates, options.properties, t);
    setFormErrors(errors);
    if (errors.length > 0) {
      return;
    }
    const write: MediaWrite = {
      isPublic: draft.isPublic,
      owner: draft.owner,
      resourceClass: draft.resourceClass,
      resourceTemplate: draft.resourceTemplate,
      thumbnail: draft.thumbnail,
      values: toValueWrites(draft.values),
      lang: nullable(lang),
      altText: nullable(altText),
    };
    save(async () => {
      onSaved(await api.updateMedia(media.id, write));
    });
  };

  return (
    <form onSubmit={submit}>
      <ResourceFields idPrefix="media" kind="media" draft={draft} onChange={setDraft} options={options} />
      <fieldset>
        <legend>{t('media.legend')}</legend>
        <p>
          <label htmlFor="media-alt-text">{t('media.altText')}</label>
          <textarea
            id="media-alt-text"
            rows={2}
            value={altText}
            onChange={(event) => {
              setAltText(event.target.value);
            }}
          />
        </p>
        <TextField id="media-lang" label={t('media.language')} value={lang} onChange={setLang} />
      </fieldset>
      <ValuesEditor
        idPrefix="media"
        draft={draft}
        onChange={setDraft}
        template={templateById(options.templates, draft.resourceTemplate)}
        properties={options.properties}
        dataTypes={options.dataTypes}
        createField={createLocalField}
      />
      <FormErrors errors={formErrors} />
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

export const MediaEditForm = ({
  media,
  onSaved,
}: {
  readonly media: Media;
  readonly onSaved: (media: Media) => void;
}): ReactElement => {
  const { t } = useTranslator(common, resources);
  const state = useAsync(loadFormOptions, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <MediaEditor media={media} options={state.data} onSaved={onSaved} />;
};
