import { useState } from "react";
import { greet } from "./api/tauri";

function App() {
  const [greeting, setGreeting] = useState<string>("");

  const handleGreet = async () => {
    try {
      const result = await greet("World");
      setGreeting(result);
    } catch (error) {
      console.error("Error calling greet command:", error);
      setGreeting("Error calling greet command");
    }
  };

  return (
    <div className="min-h-screen bg-gray-100 flex flex-col items-center justify-center p-8">
      <div className="bg-white rounded-lg shadow-lg p-8 max-w-2xl w-full">
        <h1 className="text-4xl font-bold text-gray-800 mb-6 text-center">
          Tauri + React + TypeScript
        </h1>

        <p className="text-gray-600 text-center mb-8">
          A starter template for building desktop applications
        </p>

        <div className="flex flex-col items-center gap-4">
          <button
            onClick={handleGreet}
            className="bg-primary-600 hover:bg-primary-700 text-white font-bold py-3 px-6 rounded-lg transition-colors"
          >
            Greet from Rust
          </button>

          {greeting && (
            <div className="mt-4 p-4 bg-green-50 border border-green-200 rounded-lg">
              <p className="text-green-800 font-medium">{greeting}</p>
            </div>
          )}
        </div>

        <div className="mt-8 pt-8 border-t border-gray-200">
          <h2 className="text-2xl font-semibold text-gray-800 mb-4">
            Features
          </h2>
          <ul className="space-y-2 text-gray-600">
            <li className="flex items-center">
              <span className="w-2 h-2 bg-primary-500 rounded-full mr-3"></span>
              Tauri for native desktop app development
            </li>
            <li className="flex items-center">
              <span className="w-2 h-2 bg-primary-500 rounded-full mr-3"></span>
              React for building user interfaces
            </li>
            <li className="flex items-center">
              <span className="w-2 h-2 bg-primary-500 rounded-full mr-3"></span>
              TypeScript for type safety
            </li>
            <li className="flex items-center">
              <span className="w-2 h-2 bg-primary-500 rounded-full mr-3"></span>
              Vite for fast development
            </li>
            <li className="flex items-center">
              <span className="w-2 h-2 bg-primary-500 rounded-full mr-3"></span>
              TailwindCSS for styling
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}

export default App;
