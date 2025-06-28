'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';

export default function SearchBar() {
  const [query, setQuery] = useState('');
  const [isSearching, setIsSearching] = useState(false);
  const router = useRouter();

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) return;

    setIsSearching(true);
    
    try {
      const trimmedQuery = query.trim();
      
      // Check if it's a block height (numeric)
      if (/^\d+$/.test(trimmedQuery)) {
        const height = parseInt(trimmedQuery);
        router.push(`/block/${height}`);
        return;
      }
      
      // Check if it's a transaction hash (64 hex characters)
      if (/^[a-fA-F0-9]{64}$/.test(trimmedQuery)) {
        router.push(`/tx/${trimmedQuery}`);
        return;
      }
      
      // Check if it's a block hash (64 hex characters) - same as tx for now
      if (/^[a-fA-F0-9]{64}$/.test(trimmedQuery)) {
        // Try block first, then transaction
        router.push(`/block/${trimmedQuery}`);
        return;
      }
      
      // Check if it's an address (starts with specific prefixes)
      if (/^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(trimmedQuery) || 
          /^bc1[a-z0-9]{39,59}$/.test(trimmedQuery) ||
          /^D[5-9A-HJ-NP-U][1-9A-HJ-NP-Za-km-z]{33}$/.test(trimmedQuery)) {
        // For now, show an alert since address pages aren't implemented yet
        alert('Address search coming soon!');
        return;
      }
      
      // If no pattern matches, try as block hash first
      router.push(`/block/${trimmedQuery}`);
      
    } catch (error) {
      console.error('Search error:', error);
    } finally {
      setIsSearching(false);
    }
  };

  return (
    <div className="w-full max-w-2xl mx-auto">
      <form onSubmit={handleSearch} className="relative">
        <div className="relative">
          <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
            <svg 
              className="h-5 w-5 text-gray-400" 
              xmlns="http://www.w3.org/2000/svg" 
              viewBox="0 0 20 20" 
              fill="currentColor"
            >
              <path 
                fillRule="evenodd" 
                d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z" 
                clipRule="evenodd" 
              />
            </svg>
          </div>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search by block height, block hash, transaction ID, or address..."
            className="block w-full pl-10 pr-12 py-3 border border-gray-300 rounded-lg leading-5 bg-white placeholder-gray-500 focus:outline-none focus:placeholder-gray-400 focus:ring-1 focus:ring-[color:var(--color-accent)] focus:border-[color:var(--color-accent)]"
          />
          <div className="absolute inset-y-0 right-0 flex items-center">
            <button
              type="submit"
              disabled={isSearching || !query.trim()}
              className="mr-2 inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-[color:var(--color-accent)] hover:brightness-90 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-[color:var(--color-accent)] disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isSearching ? (
                <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
              ) : (
                'Search'
              )}
            </button>
          </div>
        </div>
      </form>
      
      <div className="mt-2 text-sm text-gray-500">
        <p>Search examples:</p>
        <ul className="mt-1 space-x-4 flex flex-wrap">
          <li><code className="bg-gray-100 px-2 py-1 rounded">113255</code> (block height)</li>
          <li><code className="bg-gray-100 px-2 py-1 rounded text-xs">a8e2a4cc3a481e65...</code> (block/tx hash)</li>
        </ul>
      </div>
    </div>
  );
} 