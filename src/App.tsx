import { HashRouter, Routes, Route } from "react-router-dom";
import Layout from "./components/Layout";
import WalletPage from "./pages/WalletPage";
import WelcomePage from "./pages/WelcomePage";
import LoginPage from "./pages/LoginPage";
import MarketPage from "./pages/MarketPage";
import SellerPage from "./pages/SellerPage";
import FaqPage from "./pages/FaqPage";
import "./App.css";

export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<WelcomePage />} />
        <Route element={<Layout />}>
          <Route path="/wallet" element={<WalletPage />} />
          <Route path="/login" element={<LoginPage />} />
          <Route path="/market" element={<MarketPage />} />
          <Route path="/seller" element={<SellerPage />} />
          <Route path="/faq" element={<FaqPage />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
