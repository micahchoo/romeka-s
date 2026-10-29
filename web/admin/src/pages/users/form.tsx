import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { isUserRole } from '../../api/types';
import type { PasswordLink, User, UserId, UserRole, UserWrite } from '../../api/types';
import { useSession } from '../../app/auth';
import { DeleteControl } from '../../app/delete';
import { CheckboxField, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { ErrorNotice } from '../../app/errors';
import { LessonsSwitch } from '../../app/lessons/lesson';
import { followLessonsSwitch } from '../../app/lessons/seen';
import { LocaleSelect, localeValue } from '../../app/locale-select';
import { useSave } from '../../app/save';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { PasswordLinkPanel } from './link';
import { UserSites } from './sites';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof admin>;

/**
 * Each role's label and the line on what it may do, in upstream rank order.
 * The lines state the `omeka-core` permission matrix as the API enforces it.
 */
const ROLES: readonly {
  readonly value: UserRole;
  readonly labelKey: MessageId;
  readonly helpKey: MessageId;
}[] = [
  { value: 'global_admin', labelKey: 'role.globalAdmin', helpKey: 'roleHelp.globalAdmin' },
  { value: 'site_admin', labelKey: 'role.siteAdmin', helpKey: 'roleHelp.siteAdmin' },
  { value: 'editor', labelKey: 'role.editor', helpKey: 'roleHelp.editor' },
  { value: 'reviewer', labelKey: 'role.reviewer', helpKey: 'roleHelp.reviewer' },
  { value: 'author', labelKey: 'role.author', helpKey: 'roleHelp.author' },
  { value: 'researcher', labelKey: 'role.researcher', helpKey: 'roleHelp.researcher' },
];

const ADMIN_ROLES: ReadonlySet<string> = new Set(['global_admin', 'site_admin']);

/**
 * The roles a caller may choose: a site admin may not give an admin role
 * (`change-role-admin`), so those stay only as the user's current role.
 */
const rolesFor = (callerRole: string | null, current: string): typeof ROLES =>
  callerRole === 'site_admin'
    ? ROLES.filter((role) => !ADMIN_ROLES.has(role.value) || role.value === current)
    : ROLES;

const UserEditor = ({
  userId,
  user,
}: {
  readonly userId: UserId | null;
  readonly user: User | null;
}): ReactElement => {
  const { t } = useTranslator(common, admin);
  const session = useSession();
  // The server refuses an admin's own role, switch and delete (`IsSelf`).
  const ownAccount = user !== null && session?.id === user.id;
  const [name, setName] = useState(user?.name ?? '');
  const [email, setEmail] = useState(user?.email ?? '');
  const [role, setRole] = useState(user?.role ?? '');
  const [isActive, setIsActive] = useState(user?.isActive ?? false);
  const [localeSetting, setLocaleSetting] = useState(localeValue(user?.localeSetting ?? null));
  const [showLessons, setShowLessons] = useState(!(user?.lessons.hidden ?? false));
  const [formError, setFormError] = useState<string | null>(null);
  const [invite, setInvite] = useState<{ readonly id: UserId; readonly link: PasswordLink } | null>(
    null,
  );
  const [minting, setMinting] = useState(false);
  const [linkError, setLinkError] = useState<Error | null>(null);
  const { state: saveState, save } = useSave();
  const roleEntry = ROLES.find((entry) => entry.value === role);
  const roleHelp = roleEntry?.helpKey;

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (name.trim() === '') {
      setFormError(t('users.errorEmptyName'));
      return;
    }
    if (role === '') {
      setFormError(t('users.errorNoRole'));
      return;
    }
    setFormError(null);
    save(async () => {
      if (userId === null) {
        const write: UserWrite = { name, email, role, isActive };
        // The API's user create writes no password, so a user made here can sign
        // in only once an administrator mints the one-time key below.
        const created = await api.createUser(write);
        setInvite({ id: created.id, link: await api.createPasswordLink(created.id) });
      } else {
        const write: UserWrite = {
          name,
          email,
          role,
          isActive,
          localeSetting,
          lessonsHidden: !showLessons,
        };
        const saved = await api.updateUser(userId, write);
        setIsActive(saved.isActive);
        followLessonsSwitch(saved.id, saved.lessons.hidden);
      }
    });
  };

  /** A second link for a user who already exists, which is the reset path. */
  const mintLink = (): void => {
    if (userId === null) {
      return;
    }
    setMinting(true);
    setLinkError(null);
    api.createPasswordLink(userId).then(
      (link) => {
        setMinting(false);
        setInvite({ id: userId, link });
      },
      (error: unknown) => {
        setMinting(false);
        setLinkError(toError(error));
      },
    );
  };

  return (
    <div>
      <form onSubmit={submit}>
        <fieldset>
          <legend>{t('users.legend')}</legend>
          <TextField
            id="user-name"
            label={t('users.displayName')}
            value={name}
            onChange={setName}
          />
          <TextField
            id="user-email"
            label={t('users.email')}
            type="email"
            value={email}
            onChange={setEmail}
          />
          {ownAccount ? (
            <>
              <p>{t('users.ownRole', { role: roleEntry === undefined ? role : t(roleEntry.labelKey) })}</p>
              <p className="field-note">{t('users.ownAccount')}</p>
            </>
          ) : (
            <>
              <p>
                <label htmlFor="user-role">{t('users.role')}</label>
                <select
                  id="user-role"
                  value={role}
                  aria-describedby={roleHelp === undefined ? undefined : 'user-role-help'}
                  onChange={(event) => {
                    if (isUserRole(event.target.value)) {
                      setRole(event.target.value);
                    }
                  }}
                >
                  <option value="">{t('users.selectRole')}</option>
                  {rolesFor(session?.role ?? null, user?.role ?? '').map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {t(entry.labelKey)}
                    </option>
                  ))}
                </select>
              </p>
              {roleHelp === undefined ? null : (
                <p id="user-role-help" className="field-note">
                  {t(roleHelp)}
                </p>
              )}
              <CheckboxField
                id="user-active"
                label={t('users.active')}
                checked={isActive}
                onChange={setIsActive}
              />
            </>
          )}
        </fieldset>
        {/* Upstream's `addAction` removes the user-settings fieldset, so only an
            existing user has one. */}
        {userId === null ? null : (
          <fieldset>
            <legend>{t('users.settingsLegend')}</legend>
            <LocaleSelect id="user-locale" value={localeSetting} onChange={setLocaleSetting} />
            <LessonsSwitch id="user-lessons" checked={showLessons} onChange={setShowLessons} />
          </fieldset>
        )}
        {formError === null ? null : <p role="alert">{formError}</p>}
        <SaveStatus state={saveState} />
        <SubmitRow
          label={userId === null ? t('users.invite') : t('action.save')}
          busy={saveState.status === 'saving' || invite !== null}
        />
      </form>
      {userId === null ? null : (
        <p>
          <button type="button" disabled={minting} onClick={mintLink}>
            {t('users.passwordLink')}
          </button>
        </p>
      )}
      {linkError === null ? null : <ErrorNotice error={linkError} />}
      {invite === null ? null : (
        <>
          <PasswordLinkPanel name={name} email={email} link={invite.link} />
          {userId === null ? (
            <p>
              <a href={`#/users/${String(invite.id)}`}>{t('users.editLink')}</a>
            </p>
          ) : null}
        </>
      )}
      {user === null ? null : <UserSites userId={user.id} />}
      {user === null || ownAccount ? null : (
        <DeleteControl
          label={t('users.deleteLabel')}
          rowName={user.name}
          onConfirm={async () => {
            await api.deleteUser(user.id);
            window.location.hash = '#/users';
          }}
        />
      )}
    </div>
  );
};

/** `Admin/UserController::addAction` and `::editAction`. */
export const UserForm = ({ userId }: { readonly userId: UserId | null }): ReactElement => {
  const { t } = useTranslator(common, admin);
  const state = useAsync(
    async (): Promise<User | null> => (userId === null ? null : api.getUser(userId)),
    [userId],
  );

  if (state.status === 'loading') {
    return <p role="status">{t('loading.user')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <UserEditor
      key={userId === null ? 'new' : String(userId)}
      userId={userId}
      user={state.data}
    />
  );
};
