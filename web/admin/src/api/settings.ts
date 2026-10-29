/**
 * The global settings (`/api/settings`, a port route only a global admin may
 * use) and the time zone names its `time_zone` accepts. Only the Settings page
 * reads them, so this module stays out of the initial payload `client.ts` is
 * part of, the way `vocabularies.ts` does. The four settings every screen
 * applies have their own eager call, `ApiClient.getInstallation`.
 */
import type { ApiClient } from './client';
import { asBoolean, asIdNumber, asRecord, asString, field, optionalString, parseListOf } from './parse';
import type { GlobalSettings, GlobalSettingsWrite, Parser, Path } from './types';

const flag = (object: Record<string, unknown>, key: string, path: Path): boolean =>
  asBoolean(field(object, key, path), [...path, key]);

/** `GET /api/settings`: one map of setting keys, each as it applies. */
export const parseGlobalSettings: Parser<GlobalSettings> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const names = (key: string): readonly string[] =>
    parseListOf(asString)(field(object, key, path), [...path, key]);
  return {
    locale: optionalString(object, 'locale', path),
    mediaAltTextProperty: optionalString(object, 'media_alt_text_property', path),
    mediaTypeWhitelist: names('media_type_whitelist'),
    extensionWhitelist: names('extension_whitelist'),
    disableFileValidation: flag(object, 'disable_file_validation', path),
    importAllowPrivateUrls: flag(object, 'import_allow_private_urls', path),
    annotationsPublic: flag(object, 'romeka_annotations_public', path),
    installationTitle: optionalString(object, 'installation_title', path),
    administratorEmail: optionalString(object, 'administrator_email', path),
    versionNotifications: flag(object, 'version_notifications', path),
    timeZone: asString(field(object, 'time_zone', path), [...path, 'time_zone']),
    perPage: asIdNumber(field(object, 'pagination_per_page', path), [...path, 'pagination_per_page']),
    defaultPrivateItemSets: flag(object, 'default_to_private_item_sets', path),
    indexJob:
      object.index_fulltext_search_job === undefined
        ? null
        : asIdNumber(object.index_fulltext_search_job, [...path, 'index_fulltext_search_job']),
  };
};

export const getSettings = async (api: ApiClient): Promise<GlobalSettings> =>
  parseGlobalSettings(await api.requestJson('/api/settings'));

/**
 * `PATCH /api/settings`: store every field of the Settings page (`locale: ''`
 * removes it) and, with `indexFulltextSearch`, start the index job.
 */
export const updateSettings = async (
  api: ApiClient,
  write: GlobalSettingsWrite,
): Promise<GlobalSettings> =>
  parseGlobalSettings(
    await api.writeJson('PATCH', '/api/settings', {
      locale: write.locale ?? '',
      media_alt_text_property: write.mediaAltTextProperty ?? '',
      media_type_whitelist: write.mediaTypeWhitelist,
      extension_whitelist: write.extensionWhitelist,
      disable_file_validation: write.disableFileValidation,
      import_allow_private_urls: write.importAllowPrivateUrls,
      romeka_annotations_public: write.annotationsPublic,
      installation_title: write.installationTitle,
      administrator_email: write.administratorEmail,
      time_zone: write.timeZone,
      pagination_per_page: write.perPage,
      default_to_private_item_sets: write.defaultPrivateItemSets,
      version_notifications: write.versionNotifications,
      index_fulltext_search: write.indexFulltextSearch,
    }),
  );

/** `GET /api/romeka/time-zones`: the names a `time_zone` write accepts, in upstream's order. */
export const getTimeZones = async (api: ApiClient): Promise<readonly string[]> =>
  parseListOf(asString)(await api.requestJson('/api/romeka/time-zones'), []);
