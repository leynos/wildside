/**
 * @file Root application component.
 */
import { useQuery } from '@tanstack/react-query';
import { listUsers } from '../api/client';

export function App() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ['users'],
    queryFn: ({ signal }) => listUsers(signal),
    staleTime: 60_000,
  });

  if (isLoading) {
    return (
      <output className="p-6 min-h-screen bg-base-200 text-base-content">
        Loading users…
      </output>
    );
  }

  if (isError) {
    return (
      <p className="p-6 min-h-screen bg-base-200 text-base-content" role="alert">
        Failed to load users.
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
