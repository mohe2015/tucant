import { createSlice, PayloadAction } from "@reduxjs/toolkit";

interface UserState {
  loggedIn: boolean;
}

const initialState: UserState = {
  loggedIn: document.cookie
    .split(";")
    .some((item) => item.trim().startsWith("id=")),
};

export const userSlice = createSlice({
  name: "user",
  initialState,
  reducers: {
    setLoggedIn: (state, action: PayloadAction<boolean>) => {
      state.loggedIn = action.payload;
    },
  },
});

export const { setLoggedIn } = userSlice.actions;

export default userSlice.reducer;
