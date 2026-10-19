/**
 * Typed client for the Omeka S REST API (`/api`), with boundary parsing:
 * every response body goes through the parsers in `parse.ts` before it is
 * returned. HTTP errors map to `ApiError` and carry the upstream error body.
 */
import { localizeServerErrors } from '../i18n/serverText';
import { KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, currentKey } from './auth';
import type { ApiKey } from './auth';
import {
  assertDetached,
  parseApiErrors,
  parseApiResource,
  parseAsset,
  parseDataType,
  parseItem,
  parseItemSet,
  parseJob,
  parseListOf,
  parseMedia,
  parseModule,
  parsePasswordLink,
  parseSiteItemsChange,
  parseProperty,
  parseResourceClass,
  parseResourceTemplate,
  parseSite,
  parseSitePage,
  parseGlobalSettings,
  parseUser,
  parseVocabulary,
} from './parse';
import type {
  ApiErrors,
  ApiPage,
  ApiResource,
  Asset,
  AssetId,
  AssetWrite,
  ClassId,
  DataType,
  Item,
  ItemId,
  ItemSet,
  ItemSetId,
  ItemSetWrite,
  ItemWrite,
  Job,
  Media,
  MediaId,
  MediaWrite,
  Module,
  PageId,
  Parser,
  PasswordLink,
  SiteItemsChange,
  SiteItemsCommand,
  Property,
  PropertyId,
  ResourceClass,
  ResourceTemplate,
  ResourceTemplateWrite,
  Site,
  SiteId,
  SitePage,
  SitePageWrite,
  SiteCreate,
  TemplateId,
  User,
  UserId,
  UserSettingsWrite,
  UserWrite,
  GlobalSettings,
  Vocabulary,
  VocabularyId,
  VocabularyMemberWrite,
  VocabularyWrite,
} from './types';
import {
  encodeAsset,
  encodeItem,
  encodeItemSet,
  encodeMedia,
  encodeResourceTemplate,
  encodeSiteCreate,
  encodeSiteItemsCommand,
  encodeSitePage,
  encodeUser,
  encodeUserSettings,
  encodeVocabulary,
  encodeVocabularyMember,
} from './write';
import type { WireBody } from './write';

declare global {
  interface Window {
    /** Base URL for API requests, injected by the server. Empty means same origin. */
    readonly ROMEKA_BASE?: string;
  }
}

/** Upstream default page size for search responses (`Omeka\Paginator`). */
const DEFAULT_PER_PAGE = 25;

/**
 * A refused request. `errors` is the server's error document and `message` its
 * first message, both already in the active locale (`i18n/serverText.ts`), so
 * every surface that shows them shows the user's language.
 */
export class ApiError extends Error {
  readonly status: number;
  readonly errors: ApiErrors | null;

  constructor(status: number, errors: ApiErrors | null, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.errors = errors;
  }
}

export type LinkRel = 'first' | 'prev' | 'next' | 'last';

const LINK_RELS: ReadonlySet<string> = new Set(['first', 'prev', 'next', 'last']);

const isLinkRel = (value: string): value is LinkRel => LINK_RELS.has(value);

const LINK_PATTERN = /<([^>]*)>;\s*rel="(first|prev|next|last)"/g;

const pageFromTarget = (target: string): number | null => {
  const match = /[?&]page=(\d+)/.exec(target);
  const digits = match?.[1];
  if (digits === undefined) {
    return null;
  }
  const page = Number.parseInt(digits, 10);
  return page > 0 ? page : null;
};

/** Reads the `Link` header (`<url>; rel="next"`) into page numbers per rel. */
export const parseLinkHeader = (header: string | null): ReadonlyMap<LinkRel, number> => {
  const links = new Map<LinkRel, number>();
  if (header === null) {
    return links;
  }
  for (const match of header.matchAll(LINK_PATTERN)) {
    const target = match[1];
    const rel = match[2];
    if (target === undefined || rel === undefined || !isLinkRel(rel)) {
      continue;
    }
    const page = pageFromTarget(target);
    if (page !== null) {
      links.set(rel, page);
    }
  }
  return links;
};

/**
 * Query parameters for search endpoints. Keys are camelCase and turned into
 * upstream snake_case names (`perPage` -> `per_page`); other filters pass
 * through unchanged (`search`, `resource_class_id`, ...).
 */
export interface ResourceQuery {
  readonly page?: number;
  readonly perPage?: number;
  readonly sortBy?: string;
  readonly sortOrder?: 'asc' | 'desc';
  readonly search?: string;
  readonly itemId?: ItemId;
  readonly [param: string]: string | number | undefined;
}

const toSearchParams = (query: ResourceQuery): URLSearchParams => {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined) {
      continue;
    }
    const name = key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
    params.set(name, String(value));
  }
  return params;
};

const buildUrl = (baseUrl: string, path: string, params: URLSearchParams): string => {
  const query = params.toString();
  return query === '' ? `${baseUrl}${path}` : `${baseUrl}${path}?${query}`;
};

/**
 * Add the two parameters the API's key adapter reads, and add neither when
 * signed out. Both come from one validated pair, so a request never carries a
 * lone `key_identity`.
 */
const withApiKey = (params: URLSearchParams, key: ApiKey | null): URLSearchParams => {
  if (key !== null) {
    params.set(KEY_IDENTITY_PARAM, key.identity);
    params.set(KEY_CREDENTIAL_PARAM, key.credential);
  }
  return params;
};

const pageCount = (total: number, perPage: number): number =>
  perPage <= 0 ? 1 : Math.max(1, Math.ceil(total / perPage));

const readTotal = (header: string | null, fallback: number): number => {
  if (header === null) {
    return fallback;
  }
  const total = Number.parseInt(header, 10);
  return Number.isNaN(total) ? fallback : total;
};

const firstErrorMessage = (errors: ApiErrors | null): string | null => {
  if (errors === null) {
    return null;
  }
  const error = errors['error'];
  if (error !== undefined && error.length > 0) {
    return error[0] ?? null;
  }
  for (const messages of Object.values(errors)) {
    const first = messages[0];
    if (first !== undefined) {
      return first;
    }
  }
  return null;
};

const parseJson = (text: string): unknown => {
  const value: unknown = JSON.parse(text);
  return value;
};

const readJsonBody = async (response: Response): Promise<unknown> => {
  const text = await response.text();
  if (text === '') {
    return null;
  }
  return parseJson(text);
};

export const defaultBaseUrl = (): string => window.ROMEKA_BASE ?? '';

const defaultFetch: typeof fetch = (input, init) => globalThis.fetch(input, init);

export interface ApiClientOptions {
  readonly baseUrl?: string;
  readonly fetchFn?: typeof fetch;
}

/** The `GET /api` document plus the version header, for system info. */
export interface ApiInfo {
  readonly resources: readonly ApiResource[];
  readonly version: string | null;
}

export class ApiClient {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(options: ApiClientOptions = {}) {
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchFn = options.fetchFn ?? defaultFetch;
  }

  async listItems(query: ResourceQuery = {}): Promise<ApiPage<Item>> {
    return this.searchPage('items', query, parseItem);
  }

  async getItem(id: ItemId): Promise<Item> {
    const raw = await this.requestJson(`/api/items/${String(id)}`);
    return parseItem(raw);
  }

  async getMedia(id: MediaId): Promise<Media> {
    const raw = await this.requestJson(`/api/media/${String(id)}`);
    return parseMedia(raw);
  }

  async getItemSet(id: ItemSetId): Promise<ItemSet> {
    const raw = await this.requestJson(`/api/item_sets/${String(id)}`);
    return parseItemSet(raw);
  }

  async getUser(id: UserId): Promise<User> {
    const raw = await this.requestJson(`/api/users/${String(id)}`);
    return parseUser(raw);
  }

  async getVocabulary(id: VocabularyId): Promise<Vocabulary> {
    const raw = await this.requestJson(`/api/vocabularies/${String(id)}`);
    return parseVocabulary(raw);
  }

  async getProperty(id: PropertyId): Promise<Property> {
    const raw = await this.requestJson(`/api/properties/${String(id)}`);
    return parseProperty(raw);
  }

  async getResourceClass(id: ClassId): Promise<ResourceClass> {
    const raw = await this.requestJson(`/api/resource_classes/${String(id)}`);
    return parseResourceClass(raw);
  }

  async getResourceTemplate(id: TemplateId): Promise<ResourceTemplate> {
    const raw = await this.requestJson(`/api/resource_templates/${String(id)}`);
    return parseResourceTemplate(raw);
  }

  async createItem(write: ItemWrite): Promise<Item> {
    return parseItem(await this.writeJson('POST', '/api/items', encodeItem(write, 'create')));
  }

  async updateItem(id: ItemId, write: ItemWrite): Promise<Item> {
    return parseItem(
      await this.writeJson('PUT', `/api/items/${String(id)}`, encodeItem(write, 'update')),
    );
  }

  async createItemSet(write: ItemSetWrite): Promise<ItemSet> {
    return parseItemSet(
      await this.writeJson('POST', '/api/item_sets', encodeItemSet(write, 'create')),
    );
  }

  async updateItemSet(id: ItemSetId, write: ItemSetWrite): Promise<ItemSet> {
    return parseItemSet(
      await this.writeJson('PUT', `/api/item_sets/${String(id)}`, encodeItemSet(write, 'update')),
    );
  }

  async createUser(write: UserWrite): Promise<User> {
    return parseUser(await this.writeJson('POST', '/api/users', encodeUser(write)));
  }

  async updateUser(id: UserId, write: UserWrite): Promise<User> {
    return parseUser(await this.writeJson('PUT', `/api/users/${String(id)}`, encodeUser(write)));
  }

  /** Store the settings the write names and nothing else; `locale: ''` removes it. */
  async updateUserSettings(id: UserId, write: UserSettingsWrite): Promise<void> {
    await this.writeJson('PATCH', `/api/users/${String(id)}`, encodeUserSettings(write));
  }

  /** `GET /api/settings`, a port route only a global admin may read. */
  async getSettings(): Promise<GlobalSettings> {
    return parseGlobalSettings(await this.requestJson('/api/settings'));
  }

  /** `PATCH /api/settings`: store the global `locale`; `''` removes it. */
  async updateSettings(write: GlobalSettings): Promise<GlobalSettings> {
    return parseGlobalSettings(
      await this.writeJson('PATCH', '/api/settings', { locale: write.locale ?? '' }),
    );
  }

  async updateMedia(id: MediaId, write: MediaWrite): Promise<Media> {
    return parseMedia(await this.writeJson('PUT', `/api/media/${String(id)}`, encodeMedia(write)));
  }

  async getSite(id: SiteId): Promise<Site> {
    const raw = await this.requestJson(`/api/sites/${String(id)}`);
    return parseSite(raw);
  }

  async getSitePage(id: PageId): Promise<SitePage> {
    const raw = await this.requestJson(`/api/site_pages/${String(id)}`);
    return parseSitePage(raw);
  }

  async createSite(write: SiteCreate): Promise<Site> {
    return parseSite(await this.writeJson('POST', '/api/sites', encodeSiteCreate(write)));
  }

  /**
   * A partial site update. The site-admin sub-pages each own one member group,
   * so they `PATCH` and the server keeps every member the body omits.
   */
  async patchSite(id: SiteId, body: WireBody): Promise<Site> {
    return parseSite(await this.writeJson('PATCH', `/api/sites/${String(id)}`, body));
  }

  /**
   * Change which items a site holds (a Romeka route). A dry run answers the
   * same counts and writes nothing, which is how the admin previews a change.
   */
  async changeSiteItems(
    id: SiteId,
    command: SiteItemsCommand,
    dryRun = false,
  ): Promise<SiteItemsChange> {
    return parseSiteItemsChange(
      await this.writeJson(
        'POST',
        `/api/sites/${String(id)}/items`,
        encodeSiteItemsCommand(command, dryRun),
      ),
    );
  }

  async createSitePage(write: SitePageWrite): Promise<SitePage> {
    return parseSitePage(await this.writeJson('POST', '/api/site_pages', encodeSitePage(write)));
  }

  async updateSitePage(id: PageId, write: SitePageWrite): Promise<SitePage> {
    return parseSitePage(
      await this.writeJson('PUT', `/api/site_pages/${String(id)}`, encodeSitePage(write)),
    );
  }

  /**
   * Delete one row. `Api\Manager::finalize` renders the entity the adapter
   * removed, so the answer is the detached entity, not a live resource
   * (`crates/omeka-api/src/write/detached.rs`). Every caller discards it, so the
   * method returns nothing and the boundary only checks the detached shape.
   */
  async deleteItem(id: ItemId): Promise<void> {
    await this.deleteJson(`/api/items/${String(id)}`);
  }

  async deleteItemSet(id: ItemSetId): Promise<void> {
    await this.deleteJson(`/api/item_sets/${String(id)}`);
  }

  async deleteMedia(id: MediaId): Promise<void> {
    await this.deleteJson(`/api/media/${String(id)}`);
  }

  async deleteUser(id: UserId): Promise<void> {
    await this.deleteJson(`/api/users/${String(id)}`);
  }

  async deleteSite(id: SiteId): Promise<void> {
    await this.deleteJson(`/api/sites/${String(id)}`);
  }

  async deleteSitePage(id: PageId): Promise<void> {
    await this.deleteJson(`/api/site_pages/${String(id)}`);
  }

  async getAsset(id: AssetId): Promise<Asset> {
    return parseAsset(await this.requestJson(`/api/assets/${String(id)}`));
  }

  /**
   * `POST /api/assets` (`AssetAdapter::hydrate` on create). The body is
   * multipart: a JSON `data` part and a `file` part, which `ApiController::
   * processPostData` reads. `FormData` builds the body and its boundary; setting
   * `Content-Type` by hand would drop the boundary the parser needs.
   */
  async createAsset(write: AssetWrite, file: File): Promise<Asset> {
    const body = new FormData();
    body.set('data', JSON.stringify(encodeAsset(write)));
    body.set('file', file);
    const response = await this.fetchFn(this.apiUrl('/api/assets'), {
      method: 'POST',
      headers: { Accept: 'application/json' },
      body,
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await this.apiError(response);
    }
    return parseAsset(await readJsonBody(response));
  }

  /** `AssetAdapter::hydrate` writes `o:name` and `o:alt_text` on an update. */
  async updateAsset(id: AssetId, write: AssetWrite): Promise<Asset> {
    return parseAsset(await this.writeJson('PUT', `/api/assets/${String(id)}`, encodeAsset(write)));
  }

  async deleteAsset(id: AssetId): Promise<void> {
    await this.deleteJson(`/api/assets/${String(id)}`);
  }

  async getJob(id: Job['id']): Promise<Job> {
    return parseJob(await this.requestJson(`/api/jobs/${String(id)}`));
  }

  /**
   * `JobAdapter::hydrate` is empty, so no member of a request body reaches a
   * job's columns (`crates/omeka-api/src/write/jobs.rs`). The update finds the
   * row, checks the admin grant and answers it, which is why the body is `{}`.
   */
  async updateJob(id: Job['id']): Promise<Job> {
    return parseJob(await this.writeJson('PATCH', `/api/jobs/${String(id)}`, {}));
  }

  async deleteJob(id: Job['id']): Promise<void> {
    await this.deleteJson(`/api/jobs/${String(id)}`);
  }

  async createVocabulary(write: VocabularyWrite): Promise<Vocabulary> {
    return parseVocabulary(
      await this.writeJson('POST', '/api/vocabularies', encodeVocabulary(write, 'create')),
    );
  }

  /**
   * A partial update. A full `PUT` must carry the `o:class` and `o:property`
   * lists, and one that omits either removes every member of that kind, so the
   * vocabulary form sends the scalar members only.
   */
  async updateVocabulary(id: VocabularyId, write: VocabularyWrite): Promise<Vocabulary> {
    return parseVocabulary(
      await this.writeJson(
        'PATCH',
        `/api/vocabularies/${String(id)}`,
        encodeVocabulary(write, 'update'),
      ),
    );
  }

  async deleteVocabulary(id: VocabularyId): Promise<void> {
    await this.deleteJson(`/api/vocabularies/${String(id)}`);
  }

  async createResourceClass(write: VocabularyMemberWrite): Promise<ResourceClass> {
    return parseResourceClass(
      await this.writeJson('POST', '/api/resource_classes', encodeVocabularyMember(write, 'create')),
    );
  }

  async updateResourceClass(id: ClassId, write: VocabularyMemberWrite): Promise<ResourceClass> {
    return parseResourceClass(
      await this.writeJson(
        'PUT',
        `/api/resource_classes/${String(id)}`,
        encodeVocabularyMember(write, 'update'),
      ),
    );
  }

  async deleteResourceClass(id: ClassId): Promise<void> {
    await this.deleteJson(`/api/resource_classes/${String(id)}`);
  }

  async createProperty(write: VocabularyMemberWrite): Promise<Property> {
    return parseProperty(
      await this.writeJson('POST', '/api/properties', encodeVocabularyMember(write, 'create')),
    );
  }

  async updateProperty(id: PropertyId, write: VocabularyMemberWrite): Promise<Property> {
    return parseProperty(
      await this.writeJson(
        'PUT',
        `/api/properties/${String(id)}`,
        encodeVocabularyMember(write, 'update'),
      ),
    );
  }

  async deleteProperty(id: PropertyId): Promise<void> {
    await this.deleteJson(`/api/properties/${String(id)}`);
  }

  async createResourceTemplate(write: ResourceTemplateWrite): Promise<ResourceTemplate> {
    return parseResourceTemplate(
      await this.writeJson('POST', '/api/resource_templates', encodeResourceTemplate(write, 'create')),
    );
  }

  async updateResourceTemplate(
    id: TemplateId,
    write: ResourceTemplateWrite,
  ): Promise<ResourceTemplate> {
    return parseResourceTemplate(
      await this.writeJson(
        'PUT',
        `/api/resource_templates/${String(id)}`,
        encodeResourceTemplate(write, 'update'),
      ),
    );
  }

  async deleteResourceTemplate(id: TemplateId): Promise<void> {
    await this.deleteJson(`/api/resource_templates/${String(id)}`);
  }

  async listDataTypes(query: ResourceQuery = {}): Promise<ApiPage<DataType>> {
    return this.searchPage('data_types', query, parseDataType);
  }

  async listMediaForItem(itemId: ItemId, query: ResourceQuery = {}): Promise<ApiPage<Media>> {
    return this.searchPage('media', { itemId, ...query }, parseMedia);
  }

  async listUsers(query: ResourceQuery = {}): Promise<ApiPage<User>> {
    return this.searchPage('users', query, parseUser);
  }

  async listResourceClasses(query: ResourceQuery = {}): Promise<ApiPage<ResourceClass>> {
    return this.searchPage('resource_classes', query, parseResourceClass);
  }

  async listItemSets(query: ResourceQuery = {}): Promise<ApiPage<ItemSet>> {
    return this.searchPage('item_sets', query, parseItemSet);
  }

  async listMedia(query: ResourceQuery = {}): Promise<ApiPage<Media>> {
    return this.searchPage('media', query, parseMedia);
  }

  async listAssets(query: ResourceQuery = {}): Promise<ApiPage<Asset>> {
    return this.searchPage('assets', query, parseAsset);
  }

  async listJobs(query: ResourceQuery = {}): Promise<ApiPage<Job>> {
    return this.searchPage('jobs', query, parseJob);
  }

  async listModules(query: ResourceQuery = {}): Promise<ApiPage<Module>> {
    return this.searchPage('modules', query, parseModule);
  }

  async listProperties(query: ResourceQuery = {}): Promise<ApiPage<Property>> {
    return this.searchPage('properties', query, parseProperty);
  }

  async listVocabularies(query: ResourceQuery = {}): Promise<ApiPage<Vocabulary>> {
    return this.searchPage('vocabularies', query, parseVocabulary);
  }

  async listResourceTemplates(query: ResourceQuery = {}): Promise<ApiPage<ResourceTemplate>> {
    return this.searchPage('resource_templates', query, parseResourceTemplate);
  }

  async listSites(query: ResourceQuery = {}): Promise<ApiPage<Site>> {
    return this.searchPage('sites', query, parseSite);
  }

  async listSitePages(query: ResourceQuery = {}): Promise<ApiPage<SitePage>> {
    return this.searchPage('site_pages', query, parseSitePage);
  }

  async listApiResources(query: ResourceQuery = {}): Promise<ApiPage<ApiResource>> {
    return this.searchPage('api_resources', query, parseApiResource);
  }

  /** `GET /api` and the `Omeka-S-Version` header, for the system info page. */
  async getApiInfo(): Promise<ApiInfo> {
    const response = await this.get(this.apiUrl('/api'));
    const raw = await readJsonBody(response);
    return {
      resources: parseListOf(parseApiResource)(raw, []),
      version: response.headers.get('Omeka-S-Version'),
    };
  }

  /**
   * `LoginController::loginAction`. The answer sets the `romeka_session` cookie,
   * which is `httpOnly`, so the caller learns the user from the body and never
   * from the cookie. 401 is a wrong email or password, 403 a deactivated user
   * and 429 the login limit, and each arrives as an `ApiError`.
   */
  async login(email: string, password: string): Promise<User> {
    const response = await this.authPost('/api/login', { email, password });
    return parseUser(await readJsonBody(response));
  }

  /** `LoginController::logoutAction`: revokes the session row and clears the cookie. */
  async logout(): Promise<void> {
    await this.authPost('/api/logout', {});
  }

  /**
   * The signed-in user, or `null` when the caller is anonymous. The route
   * answers 401 for an anonymous caller, which is a state and not a failure, so
   * only any other error throws.
   */
  async getMe(): Promise<User | null> {
    const response = await this.authGet('/api/me');
    if (response.status === 401) {
      return null;
    }
    if (!response.ok) {
      throw await this.apiError(response);
    }
    return parseUser(await readJsonBody(response));
  }

  /**
   * `LoginController::createPasswordAction`. The one-time key comes from a mail
   * link or an administrator; a 404 is an unknown or expired key and a 422 a
   * password the rules reject, and the caller renders both.
   */
  async setPassword(key: string, password: string): Promise<void> {
    await this.authPost('/api/password', { key, password });
  }

  /**
   * `LoginController::forgotPasswordAction`. The route answers 204 for a known
   * and an unknown address alike, so nothing here distinguishes them.
   */
  async forgotPassword(email: string): Promise<void> {
    await this.authPost('/api/password/forgot', { email });
  }

  /**
   * Mint the one-time password key for a user. An administrator reads it and
   * hands over `link` when no SMTP service is configured; `sent` says whether
   * the invitation also went out by mail.
   */
  async createPasswordLink(id: UserId): Promise<PasswordLink> {
    const response = await this.authPost(`/api/users/${String(id)}/password-link`, {});
    return parsePasswordLink(await readJsonBody(response));
  }

  /**
   * The auth routes answer without an API key, so their URL is the base plus the
   * path, and `credentials: 'same-origin'` is what makes the session cookie ride
   * along. The browser adds the `Origin` header on a write, which is the
   * same-origin marker the server checks.
   */
  private authUrl(path: string): string {
    return `${this.baseUrl}${path}`;
  }

  private async authGet(path: string): Promise<Response> {
    return this.fetchFn(this.authUrl(path), {
      method: 'GET',
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
    });
  }

  private async authPost(path: string, body: WireBody): Promise<Response> {
    const response = await this.fetchFn(this.authUrl(path), {
      method: 'POST',
      headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await this.apiError(response);
    }
    return response;
  }

  /** A `/api` URL with the signed-in key attached, or without one when signed out. */
  private apiUrl(path: string, params: URLSearchParams = new URLSearchParams()): string {
    return buildUrl(this.baseUrl, path, withApiKey(params, currentKey()));
  }

  private async searchPage<T>(
    resource: string,
    query: ResourceQuery,
    parse: Parser<T>,
  ): Promise<ApiPage<T>> {
    const params = toSearchParams(query);
    const response = await this.get(this.apiUrl(`/api/${resource}`, params));
    const raw = await readJsonBody(response);
    const items = parseListOf(parse)(raw, []);
    const total = readTotal(response.headers.get('Omeka-S-Total-Results'), items.length);
    const perPage = query.perPage ?? DEFAULT_PER_PAGE;
    const links = parseLinkHeader(response.headers.get('Link'));
    return {
      items,
      total,
      page: query.page ?? 1,
      perPage,
      lastPage: links.get('last') ?? pageCount(total, perPage),
    };
  }

  private async requestJson(path: string): Promise<unknown> {
    const response = await this.get(this.apiUrl(path));
    return readJsonBody(response);
  }

  private async writeJson(
    method: 'POST' | 'PUT' | 'PATCH',
    path: string,
    body: WireBody,
  ): Promise<unknown> {
    const response = await this.fetchFn(this.apiUrl(path), {
      method,
      headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await this.apiError(response);
    }
    return readJsonBody(response);
  }

  private async deleteJson(path: string): Promise<void> {
    const response = await this.fetchFn(this.apiUrl(path), {
      method: 'DELETE',
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await this.apiError(response);
    }
    assertDetached(await readJsonBody(response));
  }

  private async get(url: string): Promise<Response> {
    const response = await this.fetchFn(url, {
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await this.apiError(response);
    }
    return response;
  }

  private async apiError(response: Response): Promise<ApiError> {
    const text = await response.text().catch(() => '');
    const parsed = text === '' ? null : parseApiErrors(parseJson(text));
    const errors = parsed === null ? null : await localizeServerErrors(parsed);
    const message = firstErrorMessage(errors) ?? `HTTP ${String(response.status)}`;
    return new ApiError(response.status, errors, message);
  }
}

/** The application-wide client. Tests stub `globalThis.fetch` before use. */
export const api = new ApiClient();
