/**
 * The page canvas and its public types. The editor shell imports
 * `PageCanvas`, hands it the draft and the selection, and receives every edit
 * as a `DraftAction` through `dispatch`.
 */
export { PageCanvas } from './PageCanvas';
export type { PageCanvasProps } from './PageCanvas';
