/**
 * @file Root application component.
 */
import { useQuery } from '@tanstack/react-query';
import { listUsers } from '../api/client';

export function App() {
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ['users'],
    queryFn: listUsers,
    staleTime: 60_000,
  });

  if (isLoading) {
    return (
      <p
        className="p-6 min-h-screen bg-base-200 text-base-content"
        role="status"
        aria-live="polite"
      >
        Loading users…
      </p>
    );
  }

  if (isError) {
    if (import.meta.env.DEV) {
      // eslint-disable-next-line no-console
      console.error({ msg: 'Failed to load users', error });
    }
    return (
      <p className="p-6 min-h-screen bg-base-200 text-base-content" role="alert">
        Failed to load users. Please try again.
      </p>
    );
  }

  return (
    <div className="p-6 min-h-screen bg-base-200 text-base-content">
      <div className="navbar bg-base-100 rounded-box mb-6">
        <a className="btn btn-ghost text-xl" href="/">
          myapp
        </a>
      </div>
      <ul className="menu bg-base-100 rounded-box">
        {data && data.length > 0 ? (
          data.map(u => (
            <li key={u.id}>
              <span>{u.display_name}</span>
            </li>
          ))
        ) : (
          <li>
            <span>No users found.</span>
          </li>
        )}
      </ul>
    </div>
  );
}
