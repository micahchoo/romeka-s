import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { PasswordLink, User, UserId, UserWrite } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { CheckboxField, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { ErrorNotice } from '../../app/errors';
import { useSave } from '../../app/save';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';
import { PasswordLinkPanel } from './link';

/** `Omeka\Permissions\Acl` role ids and labels, in upstream rank order. */
const ROLES: readonly { readonly value: string; readonly labelKey: MessageKey }[] = [
  { value: 'global_admin', labelKey: 'role.globalAdmin' },
  { value: 'site_admin', labelKey: 'role.siteAdmin' },
  { value: 'editor', labelKey: 'role.editor' },
  { value: 'reviewer', labelKey: 'role.reviewer' },
  { value: 'author', labelKey: 'role.author' },
  { value: 'researcher', labelKey: 'role.researcher' },
];

const isRole = (value: string): boolean => ROLES.some((role) => role.value === value);

const UserEditor = ({
  userId,
  user,
}: {
  readonly userId: UserId | null;
  readonly user: User | null;
}): ReactElement => {
  const { t } = useTranslator();
  const [name, setName] = useState(user?.name ?? '');
  const [email, setEmail] = useState(user?.email ?? '');
  const [role, setRole] = useState(user?.role ?? '');
  const [isActive, setIsActive] = useState(user?.isActive ?? false);
  const [formError, setFormError] = useState<string | null>(null);
  const [invite, setInvite] = useState<{ readonly id: UserId; readonly link: PasswordLink } | null>(
    null,
  );
  const [minting, setMinting] = useState(false);
  const [linkError, setLinkError] = useState<Error | null>(null);
  const { state: saveState, save } = useSave();

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
    const write: UserWrite = { name, email, role, isActive };
    save(async () => {
      if (userId === null) {
        // The API's user create writes no password, so a user made here can sign
        // in only once an administrator mints the one-time key below.
        const created = await api.createUser(write);
        setInvite({ id: created.id, link: await api.createPasswordLink(created.id) });
      } else {
        const saved = await api.updateUser(userId, write);
        setIsActive(saved.isActive);
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
          <p>
            <label htmlFor="user-role">{t('users.role')}</label>
            <select
              id="user-role"
              value={role}
              onChange={(event) => {
                if (isRole(event.target.value)) {
                  setRole(event.target.value);
                }
              }}
            >
              <option value="">{t('users.selectRole')}</option>
              {ROLES.map((entry) => (
                <option key={entry.value} value={entry.value}>
                  {t(entry.labelKey)}
                </option>
              ))}
            </select>
          </p>
          <CheckboxField
            id="user-active"
            label={t('users.active')}
            checked={isActive}
            onChange={setIsActive}
          />
        </fieldset>
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
      {user === null ? null : (
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
  const { t } = useTranslator();
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
