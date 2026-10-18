import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Media, MediaWrite } from '../../api/types';
import { CheckboxField, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { useSave } from '../../app/save';
import { useTranslator } from '../../i18n';

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

/**
 * `Admin/MediaController::editAction` reduced to the members this API accepts
 * and the read representation carries: alt text, language and visibility.
 * The class, template and thumbnail travel unchanged so a full `PUT` does not
 * clear them.
 */
export const MediaEditForm = ({
  media,
  onSaved,
}: {
  readonly media: Media;
  readonly onSaved: (media: Media) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const [altText, setAltText] = useState(media.altText ?? '');
  const [lang, setLang] = useState(media.lang ?? '');
  const [isPublic, setIsPublic] = useState(media.isPublic);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const write: MediaWrite = {
      isPublic,
      resourceClass: media.resourceClass?.id ?? null,
      resourceTemplate: media.resourceTemplate?.id ?? null,
      thumbnail: media.thumbnail?.id ?? null,
      lang: nullable(lang),
      altText: nullable(altText),
    };
    save(async () => {
      onSaved(await api.updateMedia(media.id, write));
    });
  };

  return (
    <form onSubmit={submit}>
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
        <CheckboxField
          id="media-public"
          label={t('common.public')}
          checked={isPublic}
          onChange={setIsPublic}
        />
      </fieldset>
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};
