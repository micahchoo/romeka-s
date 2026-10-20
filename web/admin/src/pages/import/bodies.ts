/**
 * The body of each screen, by mode. A body is a lazy chunk, fetched the first
 * time its screen opens, so the import route stays small as screens grow. The
 * shell draws the frame of a screen whose body is not written yet.
 */
import type { ComponentType } from 'react';
import type { AdvancedScreen, SimpleScreen } from './screens';

export type Bodies<S extends string> = Readonly<Partial<Record<S, ComponentType>>>;
export type SimpleBodies = Bodies<SimpleScreen>;
export type AdvancedBodies = Bodies<AdvancedScreen>;
