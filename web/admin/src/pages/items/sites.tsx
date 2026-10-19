/**
 * The item form's Sites control: the sites the signed-in user may add items
 * to, with the item's membership checked.
 */
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { currentUser } from '../../api/session';
import type { Site, SiteId, User } from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { CheckboxField } from '../../app/fields';
import { useItemText } from './messages';

const ADMIN_ROLES: readonly string[] = ['global_admin', 'site_admin'];
const ASSIGNING_SITE_ROLES: readonly string[] = ['admin', 'editor'];

/**
 * `Acl::userIsAllowed($site, 'can-assign-items')` (AclFactory.php:243), as
 * `can_assign` in `crates/omeka-api/src/write/items.rs` decides it. With no
 * session known every readable site is offered, and the server's rule still
 * decides what the write changes.
 */
const canAssignItems = (site: Site, user: User | null): boolean =>
  user === null ||
  ADMIN_ROLES.includes(user.role) ||
  site.owner?.id === user.id ||
  site.sitePermissions.some(
    (permission) =>
      permission.user.id === user.id && ASSIGNING_SITE_ROLES.includes(permission.role),
  );

export const loadAssignableSites = async (): Promise<readonly Site[]> => {
  const sites = await allPages((page) =>
    api.listSites({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'title' }),
  );
  const user = currentUser();
  return sites.filter((site) => canAssignItems(site, user));
};

/**
 * `chosen` is the whole membership the form will send. A site outside
 * `assignable` has no checkbox, so it stays as it was.
 */
export const SitesFieldset = ({
  assignable,
  chosen,
  creating,
  onChange,
}: {
  readonly assignable: readonly Site[];
  readonly chosen: readonly SiteId[];
  readonly creating: boolean;
  readonly onChange: (chosen: readonly SiteId[]) => void;
}): ReactElement => {
  const text = useItemText();
  return (
    <fieldset>
      <legend>{text('itemSites.legend')}</legend>
      {creating ? <p className="field-help">{text('itemSites.createHelp')}</p> : null}
      {assignable.length === 0 ? <p>{text('itemSites.none')}</p> : null}
      {assignable.map((site) => (
        <CheckboxField
          key={String(site.id)}
          id={`item-site-${String(site.id)}`}
          label={site.title}
          checked={chosen.includes(site.id)}
          onChange={(checked) => {
            onChange(checked ? [...chosen, site.id] : chosen.filter((id) => id !== site.id));
          }}
        />
      ))}
    </fieldset>
  );
};
