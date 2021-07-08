import React from 'react';

export function ShowWhenSdk(props) {
  if (props.middleware) {
    console.log(props);
    debugger;
    return <>{props.children}</>;
  } else {
    return null;
  }
}

export function ShowWhenProxy(props) {
  if (props.proxy) {
    return <>{props.children}</>;
  } else {
    return null;
  }
}
