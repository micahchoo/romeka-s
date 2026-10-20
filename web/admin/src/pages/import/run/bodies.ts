/**
 * The run screen's body, one lazy chunk that both modes register: Simple as
 * `add`, Advanced as `run`.
 */
import { lazy } from 'react';

export const RUN_BODY = lazy(() => import('./run-screen'));
