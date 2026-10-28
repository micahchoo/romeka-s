/**
 * The jump-to box's answer, under the box: one match opens at once, several list here, none says
 * so. Its own chunk, fetched on the first jump, so the shell carries only the box.
 */
import type { ReactElement } from 'react';
import { useEffect } from 'react';
import { useTranslator } from '../i18n';
import { finding } from '../i18n/namespaces/finding';
import { findRecord, MOST_SHOWN } from './jump-lookup';
import { recordHref } from './record-href';
import { useAsync } from './useAsync';
import './jump.css';

export interface JumpAnswerProps {
  readonly text: string;
  /** The answer opened a record, or the reader followed or closed the list. */
  readonly onDone: () => void;
}

const JumpAnswer = ({ text, onDone }: JumpAnswerProps): ReactElement | null => {
  const { t, number } = useTranslator(finding);
  const state = useAsync(() => findRecord(text), [text]);
  const opened = state.status === 'ready' && state.data.kind === 'open' ? recordHref(state.data.record) : null;
  useEffect(() => {
    if (opened !== null) {
      window.location.hash = opened;
      onDone();
    }
  }, [opened, onDone]);

  if (state.status === 'loading') {
    return (
      <p className="jump-answer" role="status">
        {t('jump.loading', { text })}
      </p>
    );
  }
  if (state.status === 'error') {
    return (
      <p className="jump-answer" role="alert">
        {t('jump.failed', { message: state.error.message })}
      </p>
    );
  }
  const answer = state.data;
  if (answer.kind === 'open') {
    return null;
  }
  if (answer.kind === 'none') {
    return (
      <p className="jump-answer" role="status">
        {t('jump.none', { text })}
      </p>
    );
  }
  const label = t('jump.many', { count: number(answer.total), text });
  return (
    <div className="jump-answer" role="status">
      <p className="jump-answer-title">{label}</p>
      <ul className="jump-answer-list" aria-label={label}>
        {answer.records.map((record) => (
          <li key={`${record.kind}-${String(record.id)}`}>
            <a href={recordHref(record)} onClick={onDone}>
              {record.title ?? t('jump.untitled')}
            </a>{' '}
            <span className="jump-answer-kind">
              {t(`jump.kind.${record.kind}`)} #{String(record.id)}
            </span>
          </li>
        ))}
      </ul>
      {answer.total > MOST_SHOWN ? <p className="jump-answer-more">{t('jump.firstTen', { count: number(MOST_SHOWN) })}</p> : null}
      <button type="button" className="jump-answer-close" onClick={onDone}>
        {t('jump.close')}
      </button>
    </div>
  );
};

export default JumpAnswer;
