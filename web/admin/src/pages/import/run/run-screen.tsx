/**
 * The last screen of an import in both modes (Simple's Add, Advanced's Run):
 * the stage the import is at, read again while it works. Simple gives its own
 * sentences for the check (`summary`); Advanced shows the counts by record
 * kind.
 */
import type { ReactElement } from 'react';
import { useDraft } from '../draft-context';
import { Ended } from './ended';
import { Ready } from './ready';
import type { Summary } from './ready';
import { stageOf } from './stage';
import { Undone } from './undone';
import { useLiveImport } from './use-live-import';
import { Working } from './working';
import './run.css';

const RunScreen = ({ summary }: { readonly summary?: Summary }): ReactElement => {
  const live = useLiveImport(useDraft());
  const stage = stageOf(live.detail);
  switch (stage.kind) {
    case 'ready':
      return <Ready live={live} summary={summary} />;
    case 'working':
      return <Working stage={stage} live={live} />;
    case 'ended':
      return <Ended stage={stage} live={live} />;
    case 'undone':
      return <Undone stage={stage} live={live} />;
  }
};

export default RunScreen;
