/**
 * The settings every admin screen applies: the installation title, the browse
 * page size, the time zone of every printed date and whether a new item set
 * starts private. The shell loads them once, before the first route renders,
 * so a list asks for its first page at the right size. The Settings page
 * loads them again after a save.
 */
import { useSyncExternalStore } from 'react';
import { api } from '../api/client';
import type { Installation } from '../api/types';

/** What applies when the server cannot answer: upstream's defaults, the port's name. */
const UNREAD: Installation = {
  title: 'Romeka S',
  perPage: 25,
  timeZone: 'UTC',
  defaultPrivateItemSets: false,
};

const listeners = new Set<() => void>();
let current = UNREAD;
let first: Promise<void> | null = null;

const read = (): Promise<void> =>
  api.getInstallation().then(
    (next) => {
      current = next;
      for (const listener of listeners) {
        listener();
      }
    },
    // An unreachable server leaves the defaults; the screens still work.
    (error: unknown) => {
      console.error(error);
    },
  );

/** The settings as last read, the defaults before then. */
export const installation = (): Installation => current;

/** Read the settings once per page load; every caller shares the one request. */
export const loadInstallation = (): Promise<void> => (first ??= read());

/** Read the settings again, after the Settings page stored new ones. */
export const reloadInstallation = (): Promise<void> => read();

const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const useInstallation = (): Installation =>
  useSyncExternalStore(subscribe, installation, installation);
