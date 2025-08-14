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
      <div className="p-6 min-h-screen bg-base-200 text-base-content" role="status">
        Loading users…
      </div>
    );
  }

  if (isError) {
    return (
      <div className="p-6 min-h-screen bg-base-200 text-base-content" role="alert">
        Failed to load users: {(error as Error).message}
      </div>
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
        {(data ?? []).map(u => (
          <li key={u.id}>
            <span>{u.display_name}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
