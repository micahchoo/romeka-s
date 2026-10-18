/**
 * The block field controls. `BlockForm` renders a block's `o:data` fields and
 * its attachments, `LayoutDataForm` renders the `o:layout_data` styling fields,
 * and both dispatch through the caller, which owns the draft.
 */
export { BlockForm } from './BlockForm';
export type { BlockFormProps } from './BlockForm';
export { LayoutDataForm } from './LayoutDataForm';
export type { LayoutDataFormProps } from './LayoutDataForm';
