import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";
import NodeStatus from "@/components/NodeStatus";
import AuthWrapper from "@/components/AuthWrapper";

export const metadata: Metadata = {
  title: "TWINS Blockchain Explorer",
  description: "Explore the TWINS blockchain network with real-time data and comprehensive statistics",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased bg-[color:var(--color-background)]">
        <AuthWrapper>
          <div className="min-h-screen flex flex-col">
            {/* Navigation Header */}
            <nav className="fixed inset-x-0 top-0 z-50 bg-white/80 backdrop-blur border-b border-gray-200">
              <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div className="flex justify-between h-16">
                  <div className="flex items-center">
                    <Link href="/" className="flex-shrink-0 flex items-center">
                      <div className="h-8 w-8 bg-[color:var(--color-accent)] rounded-lg flex items-center justify-center">
                        <span className="text-white font-bold text-sm">T</span>
                      </div>
                      <span className="ml-2 text-xl font-bold text-gray-900">TWINS Explorer</span>
                    </Link>
                    
                    <div className="hidden md:ml-6 md:flex md:space-x-8">
                      <Link
                        href="/"
                        className="text-gray-900 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium"
                      >
                        Home
                      </Link>
                      <Link
                        href="/stats"
                        className="text-gray-500 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium"
                      >
                        Network Stats
                      </Link>
                      <Link
                        href="/node"
                        className="text-gray-500 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium"
                      >
                        Node Status
                      </Link>
                    </div>
                  </div>
                  
                  <div className="flex items-center space-x-4">
                    <div className="hidden md:flex">
                      <NodeStatus compact />
                    </div>
                  </div>
                </div>
              </div>
            </nav>

            {/* Main Content */}
            <main className="flex-grow max-w-7xl mx-auto pt-20 px-4 sm:px-6 lg:px-8">
              {children}
            </main>

            {/* Footer */}
            <footer className="bg-white/80 backdrop-blur border-t border-gray-200 mt-12">
              <div className="max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-4">
                    <span className="text-sm text-gray-500">
                      © 2025 TWINS Blockchain Explorer
                    </span>
                  </div>
                  <div className="flex items-center space-x-4">
                    <span className="text-sm text-gray-500">
                      Powered by TWINS Node
                    </span>
                  </div>
                </div>
              </div>
            </footer>
          </div>
        </AuthWrapper>
      </body>
    </html>
  );
}
