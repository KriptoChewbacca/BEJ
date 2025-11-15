# Ghost Architecture: System "Duch"

## Spis treści
1. Wstęp
2. Komponenty i Funkcjonalności
   1. On-Chain: Hologram & SnipeIntentAccount
   2. Off-Chain: Seer & Trigger
3. Architektura i Przepływ Danych
4. Plan Wykonawczy: Fazy i Zadania Kluczowe
5. Techniki i Mechanizmy Implementacyjne
6. Zakończenie

---

## 1. Wstęp
Projekt **Ghost** to zaawansowana, zoptymalizowana architektura wspierająca ultra-szybkie transakcje na platformie Solana. Łączy komponenty on-chain i off-chain w celu osiągnięcia pełnej kontroli nad kluczowymi procesami arbitrażu poprzez eliminację latencji i maksymalizację niezawodności. 


---

## 2. Komponenty i Funkcjonalności

### 2.1. Komponenty On-Chain:
1. **Hologram (ghost_scheduler):**
   - Rejestruje dane intencji swapowych oraz egzekwuje je z użyciem CPI (Cross-Program Invocation).
   - Kluczowe funkcje:
        - `initialize_intent`: Tworzy i zapisuje konto intencji zawierające parametry swapu.
        - `execute_planned_swap`: Wywołuje zaplanowane transakcje przez CPI do programów AMM (np. Raydium).

2. **SnipeIntentAccount:**
    - Konto PDA (Pubkey Derived Address) przechowujące dane swapu.
    - Parametry m.in.: `amount_in`, `min_amount_out`, `pool_amm_id`.


### 2.2. Komponenty Off-Chain:
1. **Seer (Obserwator Mempoolu):**
   - Monitoruje strumień transakcji, wyszukując `InitializePool` związane z celem snipe.
   - Wysoka optymalizacja pod kątem detekcji przy minimalnym obciążeniu procesu.

2. **Trigger (Detonator):**
   - Przygotowuje i wysyła odpowiednio zoptymalizowaną "transakcję-ducha" do lidera slotu w odpowiedzi na detekcję z Seer.

---

## 3. Architektura i Przepływ Danych

#### Przebieg operacji w pętli Ghost:
1. **Pre-launch Initialization (T-60s):**
   - "Trigger" przygotowuje Transakcję-Duch.
   - "Hologram" inicjalizuje SnipeIntentAccount.

2. **Stream Detection (T=0):**
   - "Seer" skanuje dane i wysyła sygnał do "Trigger" przy znalezieniu InitializePool.

3. **Transaction Deployment (T+0.005s):**
   - "Trigger" wysyła finalną Transakcję-Ducha bezpośrednio do lidera slotu.

---

## 4. Plan Wykonawczy: Fazy i Zadania Kluczowe

### 4.1. Faza I: Przygotowanie Komponentów On-Chain

#### Zadanie 1: Stworzenie programu "Hologram"
- **Opis:** Główny program Rust służący do rejestracji intencji oraz wywołania zaplanowanych swapów.
- **Podzadania:**
    1. Utwórz funkcję `initialize_intent` z odpowiednią strukturą danych (`SnipeIntentAccount`).
    2. Zaimplementuj CPI w instrukcji `execute_planned_swap` dla programów AMM.
    3. Dodaj obsługę błędów w module `errors.rs` (np. brak dostępu, niepoprawne dane).

#### Zadanie 2: Implementacja "SnipeIntentAccount"
- **Opis:** Opracuj strukturę PDA dla przechowywania danych intencji swapu.
- **Podzadania:**
    1. Zaimplementuj serializację `borsh` dla struktury danych.
    2. Wykorzystaj derived address z `program_id` + `seeds`.

---

### 4.2 Faza II: Przygotowanie Komponentów Off-Chain

#### Zadanie 3: Implementacja Seer (Obserwator Mempoolu):
- **Podzadania:**
  1. Skonfiguruj moduł do subskrypcji mempoolu (WebSocket/Geyser).
  2. Zaimplementuj wyodrębnianie transakcji InitializePool na podstawie hash funkcji i analizy binarnej.
  3. Utwórz kanał (tokio::sync::mpsc) do przesłania sygnału do Trigger.

#### Zadanie 4: Implementacja Trigger (Detonator):
- **Podzadania:**
  1. Stwórz mechanizm składający się z przygotowania Transakcji-Duch.
  2. Wykorzystaj TpuClient do natychmiastowego wysłania do lidera slotu.
  3. Testuj synchronizację z procesem Seer.

---

### 4.3 Faza III: Testowanie i Optymalizacja

#### Zadanie 5: Testy na Devnecie
- **Podzadania:**
  1. Symuluj cykl intencji (initialize+execute) w różnych warunkach.
  2. Testuj obciążenie i wydajność w module "Seer" (mempool).
  3. Sprawdź współdziałanie Trigger z liderami slotów.

#### Zadanie 6: Benchmark w środowisku Mainnet
- **Podzadania:**
  1. Porównaj efektywność z konkurencyjnymi botami (czas detekcji vs wysyłka swapa).
  2. Skoryguj wady potencjalne w synchronizacji.

---

## 5. Techniki i Mechanizmy Implementacyjne

- **Lookup Tables:** Użyte do minimalizacji rozmiaru Transakcji-Duch (180 bajtów).
- **Deserializacja Binarna:** Optymalizuje skanowanie InitializePool.
- **TpuClient:** UDP dla bezpośredniej komunikacji z liderami slotów.
- **Pre-Symulacja:** Każda transakcja-duch jest testowana lokalnie przed wysłaniem.

---

## 6. Zakończenie
System Ghost eliminuje ryzyka opóźnień oraz problemów z wydajnością, oferując pełną niezawodność przy minimalnym obciążeniu. Dzięki wprowadzonej architekturze i praktykom bezpieczeństwa, zapewnia wyraźną przewagę konkurencyjną w wymagającej przestrzeni MEV.